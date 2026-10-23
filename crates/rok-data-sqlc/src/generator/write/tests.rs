use super::super::test_support::{column, defaulted, generated, nullable, table};
use super::*;

/// `write.rs` for `table`, dedented one level: the verbs read as rendered,
/// at column 0, though they sit in the model's `impl` (and the module's
/// forwarding wrappers lose their bodies' indent, which no test reads).
fn render(table: &Table) -> String {
    super::render(table)
        .lines()
        .map(|line| line.strip_prefix("    ").unwrap_or(line))
        .fold(String::new(), |mut output, line| {
            output.push_str(line);
            output.push('\n');
            output
        })
}

#[test]
fn static_create_when_no_column_has_a_default() {
    let rendered = render(&table(
        vec![
            column("id", "uuid::Uuid"),
            column("email", "String"),
            nullable("note", "String"),
        ],
        &["id"],
    ));
    assert!(rendered.contains("use super::models::{NewUser, UpdateUser, User};"));
    assert!(rendered.contains(
        "pub async fn create<'e, E>(executor: E, input: NewUser) -> Result<User, rok_data::Error>"
    ));
    assert!(rendered.contains(
        "    User::insert_builder()\n        .set(User::ID, input.id)\n        .set(User::EMAIL, input.email)\n        .set(User::NOTE, input.note)\n        .exec(executor)\n        .await\n"
    ));
    assert!(!rendered.contains("INSERT"));
}

#[test]
fn dynamic_create_omits_defaulted_columns_from_columns_and_binds_alike() {
    let rendered = render(&table(
        vec![
            generated(column("id", "i64")),
            column("email", "String"),
            defaulted(column("created_at", "chrono::DateTime<chrono::Utc>")),
            defaulted(nullable("tag", "String")),
        ],
        &["id"],
    ));
    assert!(rendered.contains("    let mut insert = User::insert_builder();\n"));
    assert!(rendered.contains("    insert = insert.set(User::EMAIL, input.email);\n"));
    assert!(rendered.contains("    if let Some(value) = input.created_at {\n        insert = insert.set(User::CREATED_AT, value);\n    }\n"));
    assert!(rendered.contains("    if let Some(value) = input.tag {\n        insert = insert.set(User::TAG, value);\n    }\n"));
    assert!(!rendered.contains("columns.push"));
    assert!(
        !rendered.contains("DEFAULT VALUES"),
        "a required column means the list is never empty"
    );
    assert!(rendered.contains("    insert.exec(executor).await\n"));
}

#[test]
fn dynamic_create_falls_back_to_default_values() {
    let rendered = render(&table(
        vec![
            generated(column("id", "i64")),
            defaulted(column("hits", "i32")),
            defaulted(column("created_at", "chrono::DateTime<chrono::Utc>")),
        ],
        &["id"],
    ));
    // Every column has a default, so the emitted chain may end up set-less
    // (`INSERT ... DEFAULT VALUES`, which the core emits for an empty
    // insert); the conditional sets stand in for it.
    assert!(rendered.contains("    let mut insert = User::insert_builder();\n"));
    assert!(rendered.contains("    if let Some(value) = input.hits {\n        insert = insert.set(User::HITS, value);\n    }\n"));
    assert!(rendered.contains("    if let Some(value) = input.created_at {\n        insert = insert.set(User::CREATED_AT, value);\n    }\n"));
    assert!(rendered.contains("    insert.exec(executor).await\n"));
}

#[test]
fn patch_uses_query_builder_binds_and_guards_empty_patch() {
    let rendered = render(&table(
        vec![
            column("order_id", "uuid::Uuid"),
            column("product_id", "uuid::Uuid"),
            column("quantity", "i32"),
            nullable("note", "String"),
        ],
        &["order_id", "product_id"],
    ));
    assert!(rendered.contains(
        "pub async fn patch<'e, E>(\n    executor: E,\n    key: UserKey,\n    input: UpdateUser,\n) -> Result<User, rok_data::Error>"
    ));
    // An empty patch returns the current row, or `NotFound` like any verb.
    assert!(rendered.contains(
        "    if input.is_empty() {\n        let current = super::read::get_by_id(executor, key).await?;\n        return current.ok_or_else(|| rok_data::Error::not_found::<Self>(None));\n    }\n"
    ));
    assert!(rendered.contains(
        "    let mut update = User::update_all()\n        .filter(User::ORDER_ID.eq(key.order_id))\n        .filter(User::PRODUCT_ID.eq(key.product_id));\n"
    ));
    assert!(rendered.contains("    if let Some(value) = input.quantity {\n        update = update.set(User::QUANTITY, value);\n    }\n"));
    assert!(
        !rendered.contains("assignments"),
        "the chains carry no query builder"
    );
    assert!(rendered.contains("update.returning_one_or_fail(executor).await"));
}

#[test]
fn update_assignments_follow_column_order() {
    let rendered = render(&table(
        vec![
            column("id", "i64"),
            column("alpha", "String"),
            column("beta", "String"),
            column("gamma", "String"),
        ],
        &["id"],
    ));
    let positions: Vec<usize> = ["alpha", "beta", "gamma"]
        .iter()
        .map(|name| {
            rendered
                .find(&format!(
                    "update = update.set(User::{}, value)",
                    name.to_uppercase()
                ))
                .unwrap()
        })
        .collect();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn quotes_reserved_identifiers_in_dynamic_sql() {
    let rendered = render(&table(
        vec![
            generated(column("select", "i64")),
            defaulted(column("from", "String")),
            column("user", "String"),
        ],
        &["select"],
    ));
    // The chains name every column by its Rust field, so the reserved SQL
    // names no longer appear unquoted anywhere in the module.
    assert!(!rendered.contains("\"from\""));
    assert!(!rendered.contains("\"user\" = "));
    assert!(rendered.contains("insert = insert.set(User::FROM, value);"));
    assert!(rendered.contains("insert = insert.set(User::USER, input.user);"));
    assert!(rendered.contains("update = update.set(User::USER, value);"));
    assert!(rendered.contains(".filter(User::SELECT.eq(select))"));
}

#[test]
fn destroy_fails_with_not_found_when_no_row_matched() {
    let rendered = render(&table(
        vec![column("id", "i64"), column("email", "String")],
        &["id"],
    ));
    assert!(rendered.contains(
        "pub async fn destroy<'e, E>(executor: E, id: i64) -> Result<(), rok_data::Error>"
    ));
    assert!(rendered.contains(
        "    let changed = User::filter(User::ID.eq(id)).delete(executor).await?;\n    if changed == 0 {\n        return Err(rok_data::Error::not_found::<Self>(None));\n    }\n    Ok(())\n"
    ));
}

#[test]
fn without_primary_key_only_create() {
    let rendered = render(&table(vec![column("email", "String")], &[]));
    assert!(rendered.contains("use super::models::{NewUser, User};"));
    assert!(rendered.contains("pub async fn create"));
    assert!(!rendered.contains("pub async fn patch"));
    assert!(!rendered.contains("pub async fn destroy"));
    assert!(rendered.contains("No primary key detected"));
}

#[test]
fn without_insertable_columns_skips_create() {
    let rendered = render(&table(vec![generated(column("id", "i64"))], &["id"]));
    assert!(rendered.contains("use super::models::User;"));
    assert!(!rendered.contains("pub async fn create"));
    assert!(rendered.contains("No insertable columns detected"));
    assert!(rendered.contains("No updatable columns detected"));
    assert!(rendered.contains("pub async fn destroy"));
}

#[test]
fn long_model_imports_wrap_like_rustfmt() {
    let names = |base: &str| {
        vec![
            base.to_owned(),
            format!("New{base}"),
            format!("Update{base}"),
        ]
    };
    assert_eq!(
        use_models(&names("User")[..1]),
        "use super::models::User;\n"
    );
    assert_eq!(
        use_models(&names("User")),
        "use super::models::{User, NewUser, UpdateUser};\n"
    );
    assert_eq!(
        use_models(&names("ControlledSubstanceEntry")),
        "use super::models::{\n    ControlledSubstanceEntry, NewControlledSubstanceEntry, UpdateControlledSubstanceEntry,\n};\n"
    );
    // 99 columns on one line: rustfmt already wraps it (98 is the most).
    let at_limit = [
        "ExpenseCategory",
        "ExpenseCategoryId",
        "NewExpenseCategory",
        "UpdateExpenseCategory",
    ]
    .map(str::to_owned);
    assert_eq!(
        use_models(&at_limit),
        "use super::models::{\n    ExpenseCategory, ExpenseCategoryId, NewExpenseCategory, UpdateExpenseCategory,\n};\n"
    );
    // A list too long for one indented line is packed greedily, as
    // rustfmt's mixed import layout does.
    let packed = [
        "CustomFieldDefinition",
        "CustomFieldDefinitionId",
        "NewCustomFieldDefinition",
        "UpdateCustomFieldDefinition",
    ]
    .map(str::to_owned);
    assert_eq!(
        use_models(&packed),
        "use super::models::{\n    CustomFieldDefinition, CustomFieldDefinitionId, NewCustomFieldDefinition,\n    UpdateCustomFieldDefinition,\n};\n"
    );
    let huge = "VeryLongTableNameThatKeepsGoingAndGoingAndGoing";
    assert!(use_models(&names(huge)).contains(&format!("{{\n    {huge},\n    New{huge},\n")));
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
