//! `POST /search` (engine ADR 0016, opt-in with `[api] search = true`): the body
//! in `dto.rs`, the data layer's filter less its hidden columns and
//! relationship fields, the service method, and the reads in `listing.rs`
//! that scope it to the tenant and the owner and count its rows.

use super::feature::Feature;
use super::layout::{Uses, push_call, push_signature};
use super::service::{data_args, own_params};
use crate::generator::openapi::{condition_value_type, push_to_schema, push_value_type};
use crate::generator::search::filter_columns;
use crate::generator::{doc_name, push_doc, push_ok_chain, push_wide_chain};
use crate::schema::Column;

/// The search body and its filter, with `From` into the data layer's
/// filter; returns the columns the filter's fields hold, for the imports.
pub fn render_dto<'a>(
    output: &mut String,
    uses: &mut Uses,
    feature: &Feature<'a>,
) -> Vec<&'a Column> {
    let body = feature.search_body();
    let filter = feature.search_filter();
    let sort = feature.sort_enum();
    uses.add("crate::shared::pagination::SortOrder");
    output.push('\n');
    push_doc(
        output,
        0,
        &[
            &format!(
                "`POST {}/search`'s body: `filter` matches rows by column (every",
                feature.url
            ),
            "condition must hold), `sort` and `order` sort them, and `page` and",
            "`per_page` page them, as the list's query does.",
        ],
    );
    push_search_derives(output, feature);
    let default = feature
        .default_sort()
        .map(|column| column.sql_name.clone())
        .unwrap_or_default();
    // The document's default for `filter`, which can't serialize its own.
    let filter_default = if feature.openapi {
        "    #[schema(default = json!({}))]\n"
    } else {
        ""
    };
    output.push_str(&format!(
        "pub struct {body} {{\n    /// The conditions rows must meet.\n    #[rok(nested)]\n{filter_default}    pub filter: {filter},\n    /// A `{sort}` key, `{default}` unless given.\n    #[rok(validate(custom(function = sort_key, code = \"sort_key\")))]\n    pub sort: Option<String>,\n    /// The order, `desc` unless given.\n    pub order: Option<SortOrder>,\n    /// The page, from 1.\n    pub page: Option<u32>,\n    /// The rows per page: 20 unless given, at most 100.\n    pub per_page: Option<u32>,\n}}\n"
    ));

    let db_filter = feature.db_filter();
    uses.add(&format!("{}::filter::{db_filter}", feature.db_module()));
    output.push('\n');
    push_doc(
        output,
        0,
        &[
            &format!("The search body's conditions by column: `{db_filter}` without its"),
            "relationship filters. A hidden column can't be named.",
        ],
    );
    push_search_derives(output, feature);
    output.push_str(&format!("pub struct {filter} {{\n"));
    let mut columns = Vec::new();
    let mut fields = Vec::new();
    let mut hidden = false;
    for column in filter_columns(feature.table) {
        let kind = column.filter_kind();
        let mut names = Vec::new();
        if kind.is_some() {
            names.push(column.rust_name.clone());
        }
        if column.nullable {
            names.push(column.null_check_field());
        }
        if feature.is_hidden(column) {
            for name in names {
                hidden = true;
                push_doc(
                    output,
                    4,
                    &[&format!(
                        "{} is hidden: naming it is a `422`.",
                        doc_name(&column.sql_name)
                    )],
                );
                output.push_str(&format!(
                    "    #[rok(validate(custom(function = hidden, code = \"hidden\")))]\n    pub {name}: Option<serde_json::Value>,\n"
                ));
            }
            continue;
        }
        let name = doc_name(&column.sql_name);
        if let Some(kind) = kind {
            columns.push(column);
            uses.add(&format!(
                "{}::search::{}",
                feature.db_path,
                kind.type_name()
            ));
            push_doc(output, 4, &[&format!("Conditions on the {name} column.")]);
            let field_type = format!("Vec<{}>", kind.condition_type(column));
            push_value_type(
                output,
                feature.openapi,
                condition_value_type(column, &field_type),
            );
            output.push_str(&format!("    pub {}: {field_type},\n", column.rust_name));
            fields.push(column.rust_name.clone());
        }
        if column.nullable {
            push_doc(
                output,
                4,
                &[&format!("Whether the {name} column is `NULL`.")],
            );
            output.push_str(&format!(
                "    pub {}: Option<bool>,\n",
                column.null_check_field()
            ));
            fields.push(column.null_check_field());
        }
    }
    output.push_str("}\n");

    // The data layer's filter has more fields: relationship filters,
    // `include_deleted`, and the hidden columns'.
    let rest = feature.filter_fallible || feature.soft_delete || hidden;
    let binding = if fields.is_empty() { "_" } else { "filter" };
    output.push('\n');
    output.push_str(&super::layout::impl_head(
        &format!("From<{filter}>"),
        &db_filter,
    ));
    output.push_str(&format!("    fn from({binding}: {filter}) -> Self {{\n"));
    if fields.is_empty() {
        output.push_str(&format!("        {db_filter}::default()\n"));
    } else {
        let mut lit: Vec<String> = fields
            .iter()
            .map(|field| format!("{field}: filter.{field}"))
            .collect();
        let base = format!("..{db_filter}::default()");
        if rest {
            lit.push(base.clone());
        }
        let mut block = String::new();
        super::layout::push_struct_lit(&mut block, 8, &db_filter, &lit);
        // rustfmt puts no comma after the base of a struct literal.
        output.push_str(&block.replace(&format!("{base},"), &base));
    }
    output.push_str("    }\n}\n");

    if hidden {
        output.push('\n');
        push_doc(
            output,
            0,
            &["Rejects a filter on a hidden column: the API never reveals one."],
        );
        output.push_str(
            "fn hidden(_: &serde_json::Value) -> Result<(), String> {\n    Err(\"this column can't be filtered on\".to_owned())\n}\n",
        );
    }
    output.push('\n');
    push_doc(
        output,
        0,
        &[&format!("Accepts a `sort` naming a [`{sort}`] key.")],
    );
    output.push_str(&format!(
        "fn sort_key(key: &str) -> Result<(), String> {{\n    if {sort}::from_name(key).is_some() {{\n        Ok(())\n    }} else {{\n        Err(\"not a sort key of this list\".to_owned())\n    }}\n}}\n"
    ));
    columns
}

/// The attributes of the search body and its filter, which deny unknown
/// fields; with `feature.openapi`, they derive `ToSchema`.
fn push_search_derives(output: &mut String, feature: &Feature<'_>) {
    output.push_str(
        "#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, rok_data::Input)]\n",
    );
    push_to_schema(output, feature.openapi);
    output.push_str("#[serde(default, deny_unknown_fields)]\n");
}

/// The name serde gives `variant` under `rename_all = "snake_case"`.
pub fn serde_snake_case(variant: &str) -> String {
    let mut name = String::new();
    for (index, c) in variant.chars().enumerate() {
        if c.is_uppercase() && index > 0 {
            name.push('_');
        }
        name.extend(c.to_lowercase());
    }
    name
}

/// `search_sorted`: the rows the data layer's filter matches, scoped to the
/// owner, ordered by `sort`, as a query chain the service pages.
pub fn render_listing(output: &mut String, feature: &Feature<'_>) {
    let table = feature.table;
    let row = feature.row();
    let filter = feature.db_filter();
    let mut scope = Vec::new();
    let mut args = Vec::new();
    if let Some(tenant) = table.tenant() {
        let name = crate::generator::param_name(tenant);
        scope.push(format!("{name}: {}", crate::generator::param_type(tenant)));
        args.push(name);
    }
    if feature.owner.is_some() {
        scope.push("owner: uuid::Uuid".to_owned());
    }
    let fallible = feature.filter_fallible;
    let whose = if feature.owner.is_some() {
        " `owner` owns"
    } else {
        ""
    };
    let keys = table.primary_key_columns();
    output.push_str(&format!("\nimpl {row} {{\n"));
    let mut doc = vec![
        format!(
            "The rows{whose} that `filter` matches, ordered by `sort`{}.",
            if keys.is_empty() {
                ""
            } else {
                ", ties by primary key"
            }
        ),
        "Run it a page at a time with `.paginate(executor, page, per_page)`.".to_owned(),
    ];
    if fallible {
        doc.extend(
            [
                "",
                "# Errors",
                "",
                "`rok_data::Error::InvalidQuery` when relationship filters nest more than",
                "`MAX_RELATIONSHIP_DEPTH` deep.",
            ]
            .map(str::to_owned),
        );
    }
    let doc: Vec<&str> = doc.iter().map(String::as_str).collect();
    push_doc(output, 4, &doc);
    let select = format!("rok_data::Select<{row}>");
    let returns = if fallible {
        format!("Result<{select}, rok_data::Error>")
    } else {
        output.push_str("    #[must_use]\n");
        select
    };
    let mut params = scope;
    params.push(format!("filter: {filter}"));
    params.push(format!("sort: Sort<{}>", feature.sort_enum()));
    push_signature(output, 4, "pub fn search_sorted", &params, &returns);
    args.push("filter".to_owned());
    let try_ = if fallible { "?" } else { "" };
    let mut calls = Vec::new();
    if let Some(owner) = feature.owner {
        calls.push(format!(
            ".filter({row}::{}.eq(owner))",
            owner.rust_name.to_uppercase()
        ));
    }
    calls.push(".order_by(sort)".to_owned());
    calls.extend(
        keys.iter()
            .map(|key| format!(".order_by({row}::{})", key.rust_name.to_uppercase())),
    );
    let head = format!("{row}::search({}){try_}", args.join(", "));
    if fallible {
        push_ok_chain(output, 8, &head, &calls);
    } else {
        push_wide_chain(output, 8, "", &head, &calls, "");
    }
    output.push_str("    }\n}\n");
}

/// The service method of `POST /search`: the data layer filter of the
/// body, scoped like the list, a page at a time, and its count.
pub fn render_service(output: &mut String, uses: &mut Uses, feature: &Feature<'_>) {
    uses.add(&format!("super::dto::{}", feature.search_body()));
    uses.add(&format!("super::dto::{}", feature.sort_enum()));
    uses.add(&format!("{}::search::Direction", feature.db_path));
    uses.add(&format!("{}::search::Sort", feature.db_path));
    uses.add(&format!(
        "{}::filter::{}",
        feature.db_module(),
        feature.db_filter()
    ));
    uses.add("crate::shared::pagination::Pagination");
    uses.add("crate::shared::response::PaginatedResponse");
    push_doc(
        output,
        4,
        &[&format!(
            "`POST {}/search`: one page of the rows `body.filter` matches.",
            feature.url
        )],
    );
    let mut params = own_params(feature);
    params.push(format!("body: {}", feature.search_body()));
    let returns = format!("Result<PaginatedResponse<{}>, AppError>", feature.row());
    push_signature(output, 4, "async fn search", &params, &returns);
    output.push_str("        let pagination = Pagination::new(body.page, body.per_page);\n");
    output.push_str("        let direction = Direction::from(body.order.unwrap_or_default());\n");
    output.push_str(&format!(
        "        let default = {}::DEFAULT;\n        let name = body.sort.unwrap_or_default();\n",
        feature.sort_enum()
    ));
    let callee = format!("{}::from_name", feature.sort_enum());
    push_call(output, 8, "let key = ", &callee, &["&name".to_owned()], ";");
    output.push_str("        let key = key.unwrap_or(default);\n");
    output.push_str("        let sort = Sort { key, direction };\n");
    output.push_str(&format!(
        "        let filter = {}::from(body.filter);\n",
        feature.db_filter()
    ));
    // The scope's arguments, without the executor: `search_sorted` builds
    // the query, and `paginate` runs it.
    let mut args: Vec<String> = data_args(feature, "").into_iter().skip(1).collect();
    if feature.owner.is_some() {
        args.push("user.id".to_owned());
    }
    args.extend(["filter".to_owned(), "sort".to_owned()]);
    output.push_str(
        "        let (page, per_page) = (pagination.page.into(), pagination.per_page.into());\n",
    );
    let trail = if feature.filter_fallible { "?;" } else { ";" };
    let callee = format!("{}::search_sorted", feature.row());
    push_call(output, 8, "let rows = ", &callee, &args, trail);
    output
        .push_str("        Ok(rows.paginate(self.pool(), page, per_page).await?.into())\n    }\n");
}
