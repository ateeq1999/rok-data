//! Rendering a resource's `filter.rs`: its `{Struct}Filter` and
//! `{Struct}SortKey`.

use crate::schema::{Column, FilterKind, Table, mentions_generated_type};

use super::conditions::push_conditions_fn;
use super::{
    FilterRelationship, SearchApi, filter_columns, filter_relationships, push_block, push_call,
    push_call_ending, sortable,
};
use crate::generator::sql::{ident, live_condition, str_literal};
use crate::generator::{
    HEADER, MAX_WIDTH, conventions, doc_name, keys, push_derives, push_doc, push_serde_snake_case,
    push_sort_into_order, replace_once, serde_derives, use_list, use_paths,
};

/// Renders the resource's `filter.rs`, or `None` when the resource has no
/// search function to use it. `tables` holds every table generated with it.
/// With `serde`, the filter and the sort key derive serde (engine ADR 0013).
pub fn render_filter(table: &Table, tables: &[Table], serde: bool) -> Option<String> {
    let api = SearchApi::for_table(table);
    if !api.any() {
        return None;
    }
    let filter = format!("{}Filter", table.struct_name);
    let sort_key = format!("{}SortKey", table.struct_name);
    let filterable: Vec<(&Column, FilterKind)> = filter_columns(table)
        .filter_map(|column| column.filter_kind().map(|kind| (column, kind)))
        .collect();
    let relationships = filter_relationships(table, tables);
    // A table with relationships may be the target of another's filter, so
    // its conditions get their own function, `push_conditions`.
    let split = !table.relationships.is_empty();
    let fallible = !relationships.is_empty();

    let mut output = format!("{HEADER}\n");
    let has_nullable = filter_columns(table).any(|column| column.nullable);
    let mut imports: Vec<&str> = filterable
        .iter()
        .map(|(_, kind)| kind.type_name())
        .collect();
    imports.push("Condition");
    if api.search {
        // The sort key orders by `direction`, and a `Sort` of it is an order.
        imports.extend(["Direction", "Sort"]);
    }
    if has_nullable {
        imports.push("push_null_check");
    }
    if fallible {
        imports.extend(["correlate", "open_exists"]);
    }
    imports.sort_unstable();
    imports.dedup();
    let mut paths = match imports.as_slice() {
        [] => Vec::new(),
        [single] => vec![format!("super::super::search::{single}")],
        many => vec![format!("super::super::search::{{{}}}", many.join(", "))],
    };
    // The condition is on the model's rows.
    paths.push(format!("super::models::{}", table.struct_name));
    // Typed ids appear in `Ordered<_>`/`Equality<_>` conditions only.
    paths.extend(keys::id_imports(
        table,
        filterable
            .iter()
            .filter(|(_, kind)| *kind != FilterKind::Text)
            .map(|(column, _)| *column),
        false,
    ));
    if filterable
        .iter()
        .any(|(column, _)| mentions_generated_type(column.value_type()))
    {
        paths.push("super::super::types".to_owned());
    }
    for FilterRelationship { target, .. } in &relationships {
        if target.module_name != table.module_name {
            paths.push(format!(
                "super::super::{}::filter::{}Filter",
                target.module_name, target.struct_name
            ));
        }
    }
    if !paths.is_empty() {
        let mut uses = use_paths(&paths);
        // A long list of the shared search items wraps as rustfmt wraps it.
        if let [_, _, ..] = imports.as_slice() {
            let names: Vec<String> = imports.iter().map(|name| (*name).to_owned()).collect();
            let one_line = format!("use super::super::search::{{{}}};\n", imports.join(", "));
            uses = replace_once(&uses, &one_line, &use_list("super::super::search", &names));
        }
        output.push_str(&uses);
        output.push('\n');
    }
    let title = format!("Conditions for `search` over `{}`.", table.display_name());
    let mut doc = vec![
        title.as_str(),
        "",
        "Every condition in every field must hold (they are combined with `AND`);",
        "an empty filter matches every row. A nullable column's `*_is_null` field",
        "selects rows where it is (`Some(true)`) or is not (`Some(false)`) `NULL`.",
    ];
    let live = live_condition(table, None);
    if live.is_some() {
        doc.extend([
            "Soft-deleted rows match only when `include_deleted` is set; an empty filter",
            "matches every live row.",
        ]);
    }
    if fallible {
        doc.extend([
            "",
            "A relationship field matches rows with a related row matching its filter,",
            "through an `EXISTS` subquery, so no row is returned twice; for a to-many",
            "relationship, at least one related row. An empty related filter only",
            "requires a related row: a `NULL` foreign key has none. Soft-deleted related",
            "rows never match (`include_deleted` applies to the searched table only),",
            "and a related tenant table's rows match only within the searched row's",
            "tenant. Relationship filters nest at most `MAX_RELATIONSHIP_DEPTH` deep.",
        ]);
    }
    if serde {
        doc.extend([
            "",
            "From JSON, an absent field keeps its default: no condition.",
        ]);
    }
    push_doc(&mut output, 0, &doc);
    output.push_str("#[derive(Debug, Clone, Default, PartialEq)]\n");
    push_derives(&mut output, serde_derives(serde));
    if serde {
        output.push_str("#[serde(default)]\n");
    }
    let mut fields = Vec::new();
    if live.is_some() {
        fields.push("/// Whether soft-deleted rows match too.".to_owned());
        fields.push(format!("pub {}: bool,", conventions::INCLUDE_DELETED));
    }
    for column in filter_columns(table) {
        let name = doc_name(&column.sql_name);
        if let Some(kind) = column.filter_kind() {
            fields.push(format!("/// Conditions on the {name} column."));
            fields.push(format!(
                "pub {}: Vec<{}>,",
                column.rust_name,
                kind.condition_type(column)
            ));
        }
        if column.nullable {
            fields.push(format!("/// Whether the {name} column is `NULL`."));
            fields.push(format!("pub {}: Option<bool>,", column.null_check_field()));
        }
    }
    for FilterRelationship {
        relationship,
        target,
        ..
    } in &relationships
    {
        let name = &relationship.name;
        let target_name = doc_name(&target.display_name());
        fields.push(if relationship.kind.is_to_many() {
            format!("/// Matches rows with at least one of their `{name}` ({target_name} rows) matching this filter.")
        } else {
            format!("/// Matches rows whose `{name}` ({target_name} row) exists and matches this filter.")
        });
        fields.push(format!(
            "pub {name}: Option<Box<{}Filter>>,",
            target.struct_name
        ));
    }
    push_block(&mut output, &format!("pub struct {filter}"), &fields, 0);
    output.push('\n');

    output.push_str(&format!("impl {filter} {{\n"));
    let row = &table.struct_name;
    let mut condition_doc = vec!["These conditions as one condition on the model's chain."];
    if live.is_some() {
        condition_doc.extend([
            "Soft-deleted rows are the caller's: `include_deleted` chooses",
            "`with_trashed()`.",
        ]);
    }
    let expr = format!("rok_data::Expr<{row}>");
    let returns = if fallible {
        condition_doc.extend(ERRORS_DOC);
        format!("Result<{expr}, rok_data::Error>")
    } else {
        expr
    };
    push_doc(&mut output, 4, &condition_doc);
    let params = ["self".to_owned()];
    crate::generator::push_signature(&mut output, 4, "pub(crate) fn condition", &params, &returns);
    if !split {
        let conditions =
            filter_columns(table).any(|column| column.filter_kind().is_some() || column.nullable);
        if conditions {
            output.push_str("        let mut condition = Condition::default();\n");
            output.push_str("        let query = &mut condition;\n");
            push_column_conditions(&mut output, table);
        } else {
            output.push_str("        let condition = Condition::default();\n");
        }
        output.push_str("        condition.into_expr()\n    }\n}\n");
    } else {
        output.push_str("        let mut query = Condition::default();\n");
        if fallible {
            push_call_ending(
                &mut output,
                8,
                "self.push_conditions",
                &[
                    str_literal(&ident(&table.sql_name)),
                    "0".to_owned(),
                    "&mut query".to_owned(),
                ],
                "?;",
            );
            output.push_str("        Ok(query.into_expr())\n");
        } else {
            output.push_str("        self.push_conditions(&mut query);\n");
            output.push_str("        query.into_expr()\n");
        }
        output.push_str("    }\n\n");
        push_conditions_fn(&mut output, table, tables, &relationships);
        output.push_str("}\n");
    }
    if !api.search {
        return Some(output);
    }

    output.push('\n');
    push_doc(&mut output, 0, &["Columns `search` may sort by."]);
    output.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n");
    push_derives(&mut output, serde_derives(serde));
    if serde {
        push_serde_snake_case(&mut output);
    }
    let sortable = sortable(table);
    let variants: Vec<String> = sortable
        .iter()
        .flat_map(|column| {
            [
                format!("/// Sorts by the {} column.", doc_name(&column.sql_name)),
                format!("{},", column.sort_variant()),
            ]
        })
        .collect();
    push_block(&mut output, &format!("pub enum {sort_key}"), &variants, 0);
    output.push('\n');
    output.push_str(&format!("impl {sort_key} {{\n"));
    push_doc(
        &mut output,
        4,
        &["How this key orders rows, in `direction`."],
    );
    output.push_str("    #[must_use]\n");
    let head = format!("    pub fn order(self, direction: Direction) -> rok_data::Order<{row}> {{");
    if head.len() <= MAX_WIDTH {
        output.push_str(&format!("{head}\n"));
    } else {
        output.push_str(&format!(
            "    pub fn order(\n        self,\n        direction: Direction,\n    ) -> rok_data::Order<{row}> {{\n"
        ));
    }
    let arms: Vec<String> = sortable
        .iter()
        .map(|column| {
            // The arms sit inside `fn order` and its `match`.
            const ARM_INDENT: usize = 12;
            let variant = column.sort_variant();
            let constant = format!("{row}::{}", column.rust_name.to_uppercase());
            let arm = format!("Self::{variant} => {constant},");
            // An arm too long for its line goes in a block, as rustfmt puts it.
            if ARM_INDENT + arm.len() <= MAX_WIDTH {
                arm
            } else {
                format!("Self::{variant} => {{\n                {constant}\n            }}")
            }
        })
        .collect();
    push_block(&mut output, "let column = match self", &arms, 8);
    // `push_block` closes the `match` without the `let`'s semicolon.
    output.pop();
    output.push_str(";\n");
    output.push_str(
        "        match direction {\n            Direction::Asc => column.asc(),\n            Direction::Desc => column.desc(),\n        }\n    }\n}\n",
    );
    push_sort_into_order(&mut output, &sort_key, row);
    Some(output)
}

/// The `# Errors` section of a function that follows relationship filters.
pub(super) const ERRORS_DOC: [&str; 5] = [
    "",
    "# Errors",
    "",
    "`rok_data::Error::InvalidQuery` when relationship filters nest more than",
    "`MAX_RELATIONSHIP_DEPTH` deep.",
];

/// Appends ` AND <column> <condition>` for every column condition of the
/// filter, inside a function taking `self` and `query`.
pub(super) fn push_column_conditions(output: &mut String, table: &Table) {
    for column in filter_columns(table) {
        let name = str_literal(&ident(&column.sql_name));
        if column.filter_kind().is_some() {
            output.push_str(&format!(
                "        for condition in self.{} {{\n",
                column.rust_name
            ));
            push_call(
                output,
                12,
                "condition.push",
                &[name.clone(), "query".to_owned()],
            );
            output.push_str("        }\n");
        }
        if column.nullable {
            push_call(
                output,
                8,
                "push_null_check",
                &[
                    name,
                    format!("self.{}", column.null_check_field()),
                    "query".to_owned(),
                ],
            );
        }
    }
}
