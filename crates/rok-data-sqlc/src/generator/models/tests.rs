use super::super::test_support::{column, defaulted, generated, nullable, table};
use super::*;
use crate::Conventions;
use crate::generator::conventions::apply;
use crate::schema::{Relationship, RelationshipKind, ScopeDeclaration, Through};

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
    super::render(
        &tables[0],
        DeriveOptions {
            serde: false,
            validate: false,
            openapi: false,
        },
        &tables,
    )
}

pub(super) fn render(table: &Table) -> String {
    rendered(table, &Conventions::default())
}

/// The row of `tables[0]`, rendered against sibling `tables`: the case the
/// relation attributes are for.
fn rendered_pair(mut tables: [Table; 2]) -> String {
    apply(&mut tables, &Conventions::default());
    super::render(
        &tables[0],
        DeriveOptions {
            serde: false,
            validate: false,
            openapi: false,
        },
        &tables,
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
        std::slice::from_ref(&users),
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
        std::slice::from_ref(&users),
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

/// The module and struct names a related table carries (the helper's
/// defaults are the `users` ones).
fn named(mut other: Table, module: &str, struct_name: &str) -> Table {
    other.module_name = module.to_owned();
    other.struct_name = struct_name.to_owned();
    other.sql_name = module.to_owned();
    other
}

/// The relationship side a table records, spelled out.
fn relationship(
    name: &str,
    kind: RelationshipKind,
    target: &str,
    column: &str,
    target_column: &str,
) -> Relationship {
    Relationship {
        name: name.to_owned(),
        kind,
        target: target.to_owned(),
        column: column.to_owned(),
        target_column: target_column.to_owned(),
        through: None,
    }
}

/// The foreign key a test table declares (the parser fills the name).
fn foreign_key(column: &str, referenced_table: &str) -> crate::schema::ForeignKey {
    crate::schema::ForeignKey {
        column: column.to_owned(),
        name: format!("{referenced_table}_{column}_fkey"),
        on_delete: None,
        on_update: None,
        referenced_schema: None,
        referenced_table: referenced_table.to_owned(),
        referenced_column: "id".to_owned(),
    }
}

/// A foreign-key field carries its belongs-to: the derive names the
/// relation off the column, so the attribute only names the target.
#[test]
fn a_foreign_key_field_carries_the_belongs_to() {
    let users = named(table(vec![column("id", "i64")], &["id"]), "users", "User");
    let mut posts = table(
        vec![column("id", "i64"), column("author_id", "UserId")],
        &["id"],
    );
    posts = named(posts, "posts", "Post");
    posts.relationships.push(relationship(
        "author",
        RelationshipKind::BelongsTo,
        "users",
        "author_id",
        "id",
    ));
    let rendered = rendered_pair([posts, users]);
    assert!(
        rendered.contains("#[rok(belongs_to = User)]\n    pub author_id: UserId,"),
        "{rendered}"
    );
    assert!(
        rendered.contains("use super::super::users::models::User;\n"),
        "{rendered}"
    );
}

/// A foreign key whose relation the derive cannot name (`owner`, no `_id`
/// suffix) states the recorded name itself.
#[test]
fn an_underivable_foreign_key_names_its_relation_explicitly() {
    let users = table(vec![column("id", "i64")], &["id"]);
    let mut ideas = table(
        vec![column("id", "i64"), column("owner", "UserId")],
        &["id"],
    );
    ideas = named(ideas, "ideas", "Idea");
    ideas.relationships.push(relationship(
        "owner",
        RelationshipKind::BelongsTo,
        "users",
        "owner",
        "id",
    ));
    let rendered = rendered_pair([ideas, users]);
    assert!(
        rendered.contains("#[rok(belongs_to(owner = User))]\n    pub owner: UserId,"),
        "{rendered}"
    );
}

/// The referenced table gets the inverse: a `has_many` naming the child's
/// foreign-key constant, and the child's struct imported.
#[test]
fn the_parent_gets_has_many_with_the_foreign_key_constant() {
    let mut users = table(vec![column("id", "i64")], &["id"]);
    users.relationships.push(relationship(
        "posts",
        RelationshipKind::HasMany,
        "posts",
        "id",
        "author_id",
    ));
    let posts = named(
        table(
            vec![column("id", "i64"), column("author_id", "UserId")],
            &["id"],
        ),
        "posts",
        "Post",
    );
    let rendered = rendered_pair([users.clone(), posts.clone()]);
    assert!(
        rendered.contains("#[rok(has_many(posts = Post::AUTHOR_ID))]"),
        "{rendered}"
    );
    assert!(
        rendered.contains("use super::super::posts::models::Post;\n"),
        "{rendered}"
    );
    // The child's own side comes from the same edge: the schema records the
    // relationship on the holding table, and the field carries the
    // belongs-to there.
    let mut child_of = named(
        table(
            vec![column("id", "i64"), column("author_id", "UserId")],
            &["id"],
        ),
        "posts",
        "Post",
    );
    child_of.relationships.push(relationship(
        "author",
        RelationshipKind::BelongsTo,
        "users",
        "author_id",
        "id",
    ));
    let child = rendered_pair([child_of, users]);
    assert!(child.contains("#[rok(belongs_to = User)]"), "{child}");
}

/// A foreign key that alone is a unique key at the child gives the parent a
/// `has_one`: at most one child row.
#[test]
fn a_unique_foreign_key_gives_the_parent_a_has_one() {
    let mut users = table(vec![column("id", "i64")], &["id"]);
    users.relationships.push(relationship(
        "profile",
        RelationshipKind::HasOne,
        "profiles",
        "id",
        "user_id",
    ));
    let profiles = named(
        table(
            vec![column("id", "i64"), column("user_id", "UserId")],
            &["id"],
        ),
        "profiles",
        "Profile",
    );
    let rendered = rendered_pair([users, profiles]);
    assert!(
        rendered.contains("#[rok(has_one(profile = Profile::USER_ID))]"),
        "{rendered}"
    );
}

/// A many-to-many goes through the join table's own belongs-tos: the
/// relationship itself carries no attribute (the macro has no join kind).
#[test]
fn a_many_to_many_records_no_attribute() {
    let mut users = table(vec![column("id", "i64")], &["id"]);
    users.relationships.push(Relationship {
        through: Some(Through {
            table: "group_members".to_owned(),
            column: "user_id".to_owned(),
            target_column: "group_id".to_owned(),
        }),
        name: "groups".to_owned(),
        kind: RelationshipKind::HasMany,
        target: "groups".to_owned(),
        column: "id".to_owned(),
        target_column: "id".to_owned(),
    });
    let rendered = rendered_pair([users, named(table(vec![], &[]), "groups", "Group")]);
    assert!(!rendered.contains("groups = "), "{rendered}");
    assert!(!rendered.contains("!ManyToMany"), "{rendered}");
}

/// A `parent_id` self-reference names the row's own type from inside: the
/// constants quote the struct, module imports stay out.
#[test]
fn a_self_reference_imports_nothing() {
    let mut users = table(
        vec![column("id", "i64"), column("parent_id", "UserId")],
        &["id"],
    );
    users.foreign_keys.push(foreign_key("parent_id", "users"));
    users.relationships.push(relationship(
        "parent",
        RelationshipKind::BelongsTo,
        "users",
        "parent_id",
        "id",
    ));
    users.relationships.push(relationship(
        "children",
        RelationshipKind::HasMany,
        "users",
        "id",
        "parent_id",
    ));
    let rendered = render(&users);
    assert!(
        rendered.contains("#[rok(belongs_to = User)]\n    pub parent_id: UserId,"),
        "{rendered}"
    );
    assert!(
        rendered.contains("#[rok(has_many(children = User::PARENT_ID))]"),
        "{rendered}"
    );
    assert!(!rendered.contains("super::super::users"), "{rendered}");
}

/// The `[generate.scopes]` entries for a table become the row's scope
/// attributes, one per scope, after the relation attributes.
#[test]
fn a_configured_scope_becomes_a_scope_attribute() {
    let mut users = table(vec![column("id", "i64")], &["id"]);
    users.scopes.push(ScopeDeclaration {
        name: "famous".to_owned(),
        path: "crate::scopes::users::famous".to_owned(),
    });
    users.scopes.push(ScopeDeclaration {
        name: "adults".to_owned(),
        path: "crate::scopes::users::adults".to_owned(),
    });
    let rendered = render(&users);
    assert!(
        rendered.contains("#[rok(scopes(famous = crate::scopes::users::famous))]"),
        "{rendered}"
    );
    assert!(
        rendered.contains("#[rok(scopes(adults = crate::scopes::users::adults))]"),
        "{rendered}"
    );
    // One per scope; each names the path the configuration gives.
    assert_eq!(
        rendered
            .lines()
            .filter(|line| line.contains("#[rok(scopes("))
            .count(),
        2,
        "{rendered}"
    );
}

/// A table whose configuration declares nothing carries no scope
/// attribute: the attribute is absent, not empty.
#[test]
fn a_table_without_scopes_carries_no_scope_attribute() {
    let rendered = render(&table(vec![column("id", "i64")], &["id"]));
    assert!(!rendered.contains("scopes("), "{rendered}");
}
