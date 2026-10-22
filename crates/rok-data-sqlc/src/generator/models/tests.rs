use super::super::test_support::{column, defaulted, generated, nullable, table};
use super::*;
use crate::Conventions;
use crate::generator::conventions::apply;

/// A timestamp column, with the `DEFAULT` expression it declares (none when
/// the column has no default).
fn timestamp(name: &str, default: Option<&str>) -> Column {
    let column = nullable(name, "chrono::DateTime<chrono::Utc>");
    match default {
        Some(default) => Column {
            has_default: true,
            default: Some(default.to_owned()),
            ..column
        },
        None => column,
    }
}

/// The row of `table`, with the conventions applied as the generator does.
fn rendered(table: &Table, conventions: &Conventions) -> String {
    let mut tables = [table.clone()];
    apply(&mut tables, conventions);
    render(&tables[0])
}

pub(super) fn render(table: &Table) -> String {
    super::render(
        table,
        DeriveOptions {
            serde: false,
            validate: false,
            openapi: false,
        },
    )
}

/// The timestamps the table's columns default to the current time are the
/// data layer's: the model manages them, and says nothing.
#[test]
fn a_table_with_now_defaults_gets_the_managed_timestamps() {
    let users = table(
        vec![
            column("id", "i64"),
            timestamp("created_at", Some("now()")),
            timestamp("updated_at", Some("CURRENT_TIMESTAMP")),
        ],
        &["id"],
    );
    let rendered = rendered(&users, &Conventions::default());
    assert!(
        rendered.contains("#[rok(table = \"users\")]\n"),
        "{rendered}"
    );
}

/// A timestamp column the database fills with something else keeps the
/// database's value: the model opts out.
#[test]
fn a_table_with_another_default_opts_out_of_the_managed_timestamps() {
    let users = table(
        vec![
            column("id", "i64"),
            timestamp("created_at", Some("now()")),
            timestamp("updated_at", Some("'epoch'::timestamptz")),
        ],
        &["id"],
    );
    let rendered = rendered(&users, &Conventions::default());
    assert!(
        rendered.contains("#[rok(table = \"users\", no_timestamps)]\n"),
        "{rendered}"
    );
}

/// A column the `managed_columns` convention names is the data layer's,
/// whatever its default.
#[test]
fn a_managed_column_is_managed_whatever_its_default() {
    let users = table(
        vec![
            column("id", "i64"),
            timestamp("created_at", Some("now()")),
            timestamp("updated_at", Some("'epoch'::timestamptz")),
        ],
        &["id"],
    );
    let conventions = Conventions {
        managed_columns: vec!["updated_at".to_owned()],
        ..Conventions::default()
    };
    let rendered = rendered(&users, &conventions);
    assert!(
        rendered.contains("#[rok(table = \"users\")]\n"),
        "{rendered}"
    );
}

/// A `deleted_at` column is the soft delete: the inference names it, and the
/// row says nothing.
#[test]
fn a_deleted_at_column_is_the_soft_delete() {
    let users = table(
        vec![column("id", "i64"), timestamp("deleted_at", None)],
        &["id"],
    );
    let rendered = rendered(&users, &Conventions::default());
    assert!(
        rendered.contains("#[rok(table = \"users\")]\n"),
        "{rendered}"
    );
    assert!(!rendered.contains("#[rok(deleted_at)]"), "{rendered}");
}

/// A `deleted_at` column the database fills is not the data layer's: the row
/// opts out.
#[test]
fn a_deleted_at_column_the_database_fills_opts_out_of_the_soft_delete() {
    let users = table(
        vec![column("id", "i64"), timestamp("deleted_at", Some("now()"))],
        &["id"],
    );
    let rendered = rendered(&users, &Conventions::default());
    assert!(
        rendered.contains("#[rok(table = \"users\", no_soft_delete)]\n"),
        "{rendered}"
    );
}

/// A `deleted_at` column that cannot carry a soft delete (it is not
/// nullable) is an ordinary column.
#[test]
fn a_deleted_at_column_that_cannot_carry_a_soft_delete_opts_out() {
    let users = table(
        vec![
            column("id", "i64"),
            column("deleted_at", "chrono::DateTime<chrono::Utc>"),
        ],
        &["id"],
    );
    let rendered = rendered(&users, &Conventions::default());
    assert!(
        rendered.contains("#[rok(table = \"users\", no_soft_delete)]\n"),
        "{rendered}"
    );
}

/// A `deleted_at` column the database always owns is an ordinary column.
#[test]
fn a_generated_deleted_at_column_opts_out_of_the_soft_delete() {
    let users = table(
        vec![
            column("id", "i64"),
            generated(nullable("deleted_at", "chrono::DateTime<chrono::Utc>")),
        ],
        &["id"],
    );
    let rendered = rendered(&users, &Conventions::default());
    assert!(
        rendered.contains("#[rok(table = \"users\", no_soft_delete)]\n"),
        "{rendered}"
    );
}

/// The soft-delete convention names a column the inference cannot: the row's
/// field names the column.
#[test]
fn the_soft_delete_convention_names_the_column() {
    let users = table(
        vec![column("id", "i64"), timestamp("removed_at", None)],
        &["id"],
    );
    let conventions = Conventions {
        soft_delete_column: Some("removed_at".to_owned()),
        ..Conventions::default()
    };
    let rendered = rendered(&users, &conventions);
    assert!(
        rendered.contains("#[rok(table = \"users\")]\n"),
        "{rendered}"
    );
    assert!(
        rendered.contains(
            "    /// The `removed_at` column.\n    #[rok(deleted_at)]\n    pub removed_at: Option<chrono::DateTime<chrono::Utc>>,\n"
        ),
        "{rendered}"
    );
}

/// The soft-delete convention names the `deleted_at` column: the inference
/// names it too, so the row says nothing.
#[test]
fn the_soft_delete_convention_over_a_deleted_at_column_says_nothing() {
    let users = table(
        vec![column("id", "i64"), timestamp("deleted_at", None)],
        &["id"],
    );
    let conventions = Conventions {
        soft_delete_column: Some("deleted_at".to_owned()),
        ..Conventions::default()
    };
    let rendered = rendered(&users, &conventions);
    assert!(
        rendered.contains("#[rok(table = \"users\")]\n"),
        "{rendered}"
    );
    assert!(!rendered.contains("#[rok(deleted_at)]"), "{rendered}");
}

/// One timestamp column alone is not the convention: the model would manage
/// a half-pair, so the row says nothing.
#[test]
fn one_timestamp_column_alone_says_nothing() {
    let users = table(
        vec![column("id", "i64"), timestamp("created_at", Some("now()"))],
        &["id"],
    );
    let rendered = rendered(&users, &Conventions::default());
    assert!(
        rendered.contains("#[rok(table = \"users\")]\n"),
        "{rendered}"
    );
}

/// A timestamp column the database always owns is the database's: the row
/// opts out of the managed timestamps.
#[test]
fn a_generated_timestamp_column_opts_out_of_the_managed_timestamps() {
    let users = table(
        vec![
            column("id", "i64"),
            generated(nullable("created_at", "chrono::DateTime<chrono::Utc>")),
            timestamp("updated_at", Some("now()")),
        ],
        &["id"],
    );
    let rendered = rendered(&users, &Conventions::default());
    assert!(
        rendered.contains("#[rok(table = \"users\", no_timestamps)]\n"),
        "{rendered}"
    );
}

#[test]
fn serde_derives_rows_inputs_and_patches_on_request() {
    let users = table(
        vec![column("id", "i64"), column("email", "String")],
        &["id"],
    );
    let serde = "serde::Serialize, serde::Deserialize)]\n";
    assert!(!render(&users).contains("serde"));
    let rendered = super::render(
        &users,
        DeriveOptions {
            serde: true,
            validate: false,
            openapi: false,
        },
    );
    for item in ["", "pub struct NewUser {"] {
        assert!(
            rendered.contains(&format!("{serde}{item}")),
            "{item}:\n{rendered}"
        );
    }
    // The row's derive names `Model` and the table; serde rides along in
    // the same `#[derive(...)]`.
    assert!(
        rendered.contains(
            "#[derive(Debug, Clone, PartialEq, Model, serde::Serialize, serde::Deserialize)]\n#[rok(table = \"users\")]\npub struct User {"
        ),
        "{rendered}"
    );
    assert!(
        rendered.contains(&format!(
            "{serde}#[serde(default)]\npub struct UpdateUser {{"
        )),
        "{rendered}"
    );
}

/// Plain serde reads both an absent field and `null` as `None`; the
/// generated `db::nullable` helper reads `null` as `Some(None)`.
#[test]
fn serde_patches_and_inputs_tell_absent_from_null() {
    let users = table(
        vec![
            column("id", "i64"),
            column("email", "String"),
            nullable("note", "String"),
            defaulted(nullable("tag", "String")),
        ],
        &["id"],
    );
    let rendered = super::render(
        &users,
        DeriveOptions {
            serde: true,
            validate: false,
            openapi: false,
        },
    );
    let patch = rendered.split("pub struct UpdateUser {").nth(1).unwrap();
    assert!(patch.contains(
            "    #[serde(skip_serializing_if = \"Option::is_none\")]\n    pub email: Option<String>,\n"
        ), "{patch}");
    assert!(patch.contains(
            "    #[serde(\n        deserialize_with = \"super::super::nullable::deserialize\",\n        skip_serializing_if = \"Option::is_none\"\n    )]\n    pub note: Option<Option<String>>,\n"
        ), "{patch}");

    let input = rendered.split("pub struct NewUser {").nth(1).unwrap();
    let input = input.split("\n}\n").next().unwrap();
    assert!(input.contains(
            "    #[serde(\n        default,\n        deserialize_with = \"super::super::nullable::deserialize\",\n        skip_serializing_if = \"Option::is_none\"\n    )]\n    pub tag: Option<Option<String>>,"
        ), "{input}");
    // Only the `Option<Option<T>>` input field needs the helper.
    assert_eq!(input.matches("#[serde(").count(), 1, "{input}");
}

#[test]
fn nullable_helper_reads_null_as_some_none() {
    let rendered = render_nullable();
    assert!(rendered.starts_with(HEADER));
    assert!(rendered.contains(
        "pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>"
    ));
    assert!(rendered.contains("Option::<T>::deserialize(deserializer).map(Some)"));
}

#[test]
fn renders_row_struct_with_from_row() {
    let rendered = render(&table(vec![column("email", "String")], &[]));
    assert!(rendered.starts_with("// @generated by rok-data-sqlc. Do not edit by hand.\n"));
    assert!(rendered.contains(
            "/// A row of the `users` table.\n#[derive(Debug, Clone, PartialEq, Model)]\n#[rok(table = \"users\")]\npub struct User {\n    /// The `email` column.\n    pub email: String,\n}"
        ));
}

#[test]
fn row_doc_names_schema_qualified_table() {
    let mut users = table(vec![column("email", "String")], &[]);
    users.schema = Some("core".to_owned());
    assert!(render(&users).contains("/// A row of the `core.users` table."));
}

#[test]
fn wraps_nullable_columns_in_option() {
    let rendered = render(&table(vec![nullable("email", "String")], &[]));
    assert!(rendered.contains("pub email: Option<String>,"));
}

#[test]
fn emits_rename_for_renamed_fields() {
    let mut renamed = column("type", "String");
    renamed.rust_name = "type_".to_owned();
    let mut quoted = column("Mixed \"Case\"", "String");
    quoted.rust_name = "mixed_case".to_owned();
    let rendered = render(&table(vec![renamed, quoted], &[]));
    assert!(rendered.contains("    #[rok(column = \"type\")]\n    pub type_: String,"));
    assert!(
        rendered
            .contains("    #[rok(column = r#\"Mixed \"Case\"\"#)]\n    pub mixed_case: String,")
    );
}

#[test]
fn insert_input_omits_generated_and_wraps_defaulted_columns() {
    let rendered = render(&table(
        vec![
            generated(column("id", "i64")),
            column("email", "String"),
            nullable("nickname", "String"),
            defaulted(column("created_at", "chrono::DateTime<chrono::Utc>")),
            defaulted(nullable("tag", "String")),
        ],
        &["id"],
    ));
    let input = rendered.split("pub struct NewUser {").nth(1).unwrap();
    let input = input.split('}').next().unwrap();
    assert!(!input.contains("pub id:"));
    assert!(input.contains("    pub email: String,\n"));
    assert!(input.contains("    pub nickname: Option<String>,\n"));
    assert!(input.contains("    pub created_at: Option<chrono::DateTime<chrono::Utc>>,\n"));
    assert!(input.contains(
            "    /// `None` applies the column default; `Some(None)` inserts `NULL`.\n    pub tag: Option<Option<String>>,\n"
        ));
    assert!(rendered.contains("#[derive(Debug, Clone, PartialEq)]\npub struct NewUser {"));
}

#[test]
fn insert_input_derives_default_when_every_field_is_optional() {
    let rendered = render(&table(
        vec![
            generated(column("id", "i64")),
            defaulted(column("hits", "i32")),
            nullable("note", "String"),
        ],
        &["id"],
    ));
    assert!(rendered.contains("#[derive(Debug, Clone, Default, PartialEq)]\npub struct NewUser {"));
}

#[test]
fn update_patch_documents_null_and_offers_is_empty() {
    let rendered = render(&table(
        vec![
            column("id", "i64"),
            column("email", "String"),
            nullable("note", "String"),
        ],
        &["id"],
    ));
    assert!(
        rendered.contains("#[derive(Debug, Clone, Default, PartialEq)]\npub struct UpdateUser {")
    );
    assert!(rendered.contains("    pub email: Option<String>,\n"));
    assert!(rendered.contains(
        "    /// `Some(None)` sets the column to `NULL`.\n    pub note: Option<Option<String>>,\n"
    ));
    assert!(!rendered.contains("    pub id: Option"));
    assert!(rendered.contains(
            "    pub fn is_empty(&self) -> bool {\n        self.email.is_none() && self.note.is_none()\n    }"
        ));
}

#[test]
fn omits_update_patch_without_primary_key() {
    let rendered = render(&table(vec![column("email", "String")], &[]));
    assert!(!rendered.contains("pub struct UpdateUser"));
}

#[test]
fn output_is_deterministic() {
    let build = || {
        render(&table(
            vec![column("id", "i64"), column("email", "String")],
            &["id"],
        ))
    };
    assert_eq!(build(), build());
}
