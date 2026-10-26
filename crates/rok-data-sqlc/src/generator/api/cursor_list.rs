//! A list paged by cursor (`[api] cursor`): in the data layer's
//! `listing.rs`, `list_by_cursor` (the runtime's keyset pagination over the
//! model, scoped like the numbered list) and the sort key's `order`; in the
//! service, the `list` that parses `?cursor=` and answers a
//! `CursorResponse`.

use super::feature::Feature;
use super::layout::{Uses, push_call, push_signature};
use super::service::{data_args, own_params};
use crate::generator::{max_width, model_chain, push_doc, push_fn_head, push_wide_chain};
use crate::schema::Column;

/// The column's `Model` const, such as `Note::TITLE`.
fn model_const(feature: &Feature<'_>, column: &Column) -> String {
    format!("{}::{}", feature.row(), column.rust_name.to_uppercase())
}

/// `list_by_cursor`, and `matches_q`, the condition of `q`, when the list
/// searches.
pub fn render_listing(output: &mut String, feature: &Feature<'_>) {
    let table = feature.table;
    let row = feature.row();
    let searched = !feature.search.is_empty();
    output.push('\n');
    let matching = format!(
        "{}{}",
        if feature.owner.is_some() {
            " `owner` owns"
        } else {
            ""
        },
        if searched { " matching `q`" } else { "" }
    );
    push_doc(
        output,
        0,
        &[
            &format!("Fetches one page of rows{matching}, ordered by `sort`, ties by primary key,"),
            "starting at `cursor`, the way it points (the first page without one).",
        ],
    );
    let mut params: Vec<String> = table
        .tenant()
        .map(|tenant| {
            format!(
                "{}: {}",
                crate::generator::param_name(tenant),
                crate::generator::param_type(tenant)
            )
        })
        .into_iter()
        .collect();
    params.extend(feature.owner.map(|_| "owner: uuid::Uuid".to_owned()));
    if searched {
        params.push("q: Option<&str>".to_owned());
    }
    params.extend([
        format!("sort: Sort<{}>", feature.sort_enum()),
        "cursor: Option<&rok_data::Cursor>".to_owned(),
        "per_page: u64".to_owned(),
    ]);
    push_fn_head(
        output,
        "list_by_cursor",
        &params,
        &format!("Result<rok_data::CursorPage<{row}>, rok_data::Error>"),
    );
    let mut calls = Vec::new();
    if let Some(column) = table
        .soft_delete_column
        .as_deref()
        .and_then(|name| table.columns.iter().find(|column| column.sql_name == name))
    {
        calls.push(format!(
            ".filter({}.is_null())",
            model_const(feature, column)
        ));
    }
    if let Some(tenant) = table.tenant() {
        calls.push(format!(
            ".filter({}.eq({}))",
            model_const(feature, tenant),
            crate::generator::param_name(tenant)
        ));
    }
    if let Some(owner) = feature.owner {
        calls.push(format!(
            ".filter({}.eq(owner))",
            model_const(feature, owner)
        ));
    }
    if searched {
        calls.push(".filter_opt(q, matches_q)".to_owned());
    }
    calls.extend([
        ".order_by(sort.key.order(sort.direction))".to_owned(),
        ".cursor_paginate(executor, cursor, per_page)".to_owned(),
        ".await".to_owned(),
    ]);
    let head = model_chain(row, &mut calls, 4, "");
    push_wide_chain(output, 4, "", &head, &calls, "");
    output.push_str("}\n");
    if searched {
        render_matches_q(output, feature);
    }
}

/// `matches_q`: the rows where one of the searched columns contains `q`,
/// ignoring case.
fn render_matches_q(output: &mut String, feature: &Feature<'_>) {
    let row = feature.row();
    let names: Vec<String> = feature
        .search
        .iter()
        .map(|column| format!("`{}`", column.sql_name))
        .collect();
    output.push('\n');
    push_doc(
        output,
        0,
        &[
            &format!(
                "The rows where {} contains `q`, ignoring case; `%`, `_` and `\\`",
                if let [one] = names.as_slice() {
                    one.clone()
                } else {
                    format!("one of {}", names.join(", "))
                }
            ),
            "in it match themselves.",
        ],
    );
    output.push_str(&format!(
        "fn matches_q(q: &str) -> rok_data::Expr<{row}> {{\n    let pattern = contains_pattern(q);\n"
    ));
    let items: Vec<String> = feature
        .search
        .iter()
        .map(|column| format!("{}.ilike(pattern.as_str())", model_const(feature, column)))
        .collect();
    let joined = items.join(", ");
    let one_line = format!("    rok_data::Expr::any_of([{joined}])");
    // rustfmt's `array_width`: 60.
    if joined.len() <= 60 && one_line.len() <= max_width() {
        output.push_str(&one_line);
        output.push('\n');
    } else {
        output.push_str("    rok_data::Expr::any_of([\n");
        for item in &items {
            output.push_str(&format!("        {item},\n"));
        }
        output.push_str("    ])\n");
    }
    output.push_str("}\n");
}

/// The sort key's `order`: the runtime order it sorts rows in, `direction`
/// first. Appended to the `XSort` impl.
pub fn render_order(output: &mut String, feature: &Feature<'_>) {
    output.push('\n');
    push_doc(
        output,
        4,
        &["The order this key sorts rows in, for the runtime's queries."],
    );
    output.push_str(&format!(
        "    #[must_use]\n    pub fn order(self, direction: Direction) -> rok_data::Order<{}> {{\n        let column = match self {{\n",
        feature.row()
    ));
    for column in &feature.sort {
        output.push_str(&format!(
            "            Self::{} => {},\n",
            column.sort_variant(),
            model_const(feature, column)
        ));
    }
    output.push_str(
        "        };\n        match direction {\n            Direction::Asc => column.asc(),\n            Direction::Desc => column.desc(),\n        }\n    }\n",
    );
}

/// The service's `list`: the page from `query.cursor` on, sorted and
/// matched by `query`.
pub fn render_service(output: &mut String, uses: &mut Uses, feature: &Feature<'_>) {
    uses.add(&format!("super::dto::{}", feature.list_query()));
    uses.add(&format!("super::dto::{}", feature.sort_enum()));
    uses.add(&format!("{}::search::Direction", feature.db_path));
    uses.add(&format!("{}::search::Sort", feature.db_path));
    uses.add("crate::shared::pagination::cursor_error");
    uses.add("crate::shared::response::CursorResponse");
    push_doc(
        output,
        4,
        &[&format!(
            "`GET {}`: one page of rows from `query.cursor` on, sorted and matched by `query`.",
            feature.url
        )],
    );
    let mut params = own_params(feature);
    params.push(format!("query: {}", feature.list_query()));
    push_signature(
        output,
        4,
        "async fn list",
        &params,
        &format!("Result<CursorResponse<{}>, AppError>", feature.row()),
    );
    output.push_str("        let cursor = query.cursor()?;\n");
    output.push_str("        let direction = Direction::from(query.sort_order());\n");
    output.push_str(&format!(
        "        let default = {}::DEFAULT;\n        let key = query.sort.unwrap_or(default);\n",
        feature.sort_enum()
    ));
    output.push_str("        let sort = Sort { key, direction };\n");
    let q = !feature.search.is_empty();
    if q {
        output.push_str("        let q = query.q.as_deref().filter(|q| !q.is_empty());\n");
    }
    let mut args = data_args(feature, "self.pool()");
    if feature.owner.is_some() {
        args.push("user.id".to_owned());
    }
    if q {
        args.push("q".to_owned());
    }
    args.extend([
        "sort".to_owned(),
        "cursor.as_ref()".to_owned(),
        "query.per_page()".to_owned(),
    ]);
    push_call(
        output,
        8,
        "let page = ",
        "repo::list_by_cursor",
        &args,
        ".await;",
    );
    output.push_str("        Ok(CursorResponse::from(page.map_err(cursor_error)?))\n    }\n");
}
