use super::super::test_support::{column, table};
use super::*;

/// Renders `table` as if generated on its own.
fn render_alone(table: &Table) -> String {
    render(table, &[])
}

/// `by_primary_key` is a chain on the model: the caller runs it.
#[test]
fn renders_by_primary_key_as_a_chain() {
    let rendered = render_alone(&table(
        vec![column("id", "i64"), column("email", "String")],
        &["id"],
    ));
    assert!(
        rendered.contains(
            "impl User {\n    /// The row with the given primary key, as a query chain: `.first(executor)`\n    /// fetches it, `.exists(executor)` checks it.\n    #[must_use]\n    pub fn by_primary_key(id: i64) -> rok_data::Select<User> {\n        User::filter(User::ID.eq(id))\n    }\n"
        ),
        "{rendered}"
    );
}

/// Fetching by key, every row and a count are the runtime's verbs, so the
/// engine generates none of them (engine ADR 0023).
#[test]
fn renders_no_runtime_verbs() {
    let rendered = render_alone(&table(
        vec![column("id", "i64"), column("email", "String")],
        &["id"],
    ));
    for gone in [
        "get_by_id",
        "exists_by_id",
        "fn list",
        "list_page",
        "fn count",
        "async fn",
    ] {
        assert!(!rendered.contains(gone), "`{gone}`:\n{rendered}");
    }
}

#[test]
fn composite_key_parameters_follow_key_order() {
    let rendered = render_alone(&table(
        vec![
            column("order_id", "uuid::Uuid"),
            column("product_id", "uuid::Uuid"),
            column("quantity", "i32"),
        ],
        &["order_id", "product_id"],
    ));
    assert!(
        rendered.contains("pub fn by_primary_key(key: UserKey) -> rok_data::Select<User> {"),
        "{rendered}"
    );
    let filters: Vec<usize> = ["ORDER_ID", "PRODUCT_ID"]
        .iter()
        .map(|name| {
            rendered
                .find(&format!("filter(User::{name}.eq(key."))
                .unwrap()
        })
        .collect();
    assert!(
        filters.windows(2).all(|pair| pair[0] < pair[1]),
        "filters follow key order: {rendered}"
    );
}

#[test]
fn quotes_reserved_and_mixed_case_identifiers_and_keeps_schema() {
    let mut users = table(
        vec![column("select", "i64"), column("Email", "String")],
        &["select"],
    );
    users.schema = Some("Auth".to_owned());
    let rendered = render_alone(&users);
    // The chains name columns by Rust field name; the reserved or
    // mixed-case SQL name is quoted in the row's model derive, which also
    // names the `"Auth".users` table.
    assert!(rendered.contains("filter(User::SELECT.eq(select))"));
    assert!(!rendered.contains("FROM "), "{rendered}");
    assert!(
        rendered.contains("User::filter(filter.condition())"),
        "{rendered}"
    );
}

#[test]
fn key_params_avoid_local_name_collisions() {
    let rendered = render_alone(&table(vec![column("limit", "i64")], &["limit"]));
    assert!(
        rendered.contains("pub fn by_primary_key(limit_key: i64)"),
        "{rendered}"
    );
    assert!(rendered.contains("filter(User::LIMIT.eq(limit_key))"));
}

#[test]
fn without_primary_key_only_search() {
    let rendered = render_alone(&table(vec![column("email", "String")], &[]));
    assert!(!rendered.contains("fn by_primary_key"), "{rendered}");
    // `search` is a chain: the caller orders it.
    assert!(
        rendered.contains("pub fn search(filter: UserFilter) -> rok_data::Select<User> {"),
        "{rendered}"
    );
    assert!(rendered.contains("No primary key detected"));
}

/// A text key is borrowed; `Copy` keys stay by value.
#[test]
fn text_keys_are_borrowed() {
    let mut code = column("code", "String");
    code.typed_id = Some(crate::schema::TypedId {
        module: "users".to_owned(),
        name: "UserId".to_owned(),
    });
    let rendered = render_alone(&table(vec![code, column("email", "String")], &["code"]));
    assert!(
        rendered.contains("pub fn by_primary_key(code: &UserId)"),
        "{rendered}"
    );
    assert!(rendered.contains("filter(User::CODE.eq(code))"));

    let rendered = render_alone(&table(
        vec![column("order_id", "uuid::Uuid"), column("sku", "String")],
        &["order_id", "sku"],
    ));
    assert!(
        rendered.contains("pub fn by_primary_key(key: &UserKey)"),
        "{rendered}"
    );
}

/// A `BYTEA` key is borrowed as a byte slice, like text as `&str`.
#[test]
fn byte_keys_are_borrowed_as_slices() {
    let rendered = render_alone(&table(
        vec![column("digest", "Vec<u8>"), column("email", "String")],
        &["digest"],
    ));
    assert!(
        rendered.contains("pub fn by_primary_key(digest: &[u8])"),
        "{rendered}"
    );
    assert!(rendered.contains("filter(User::DIGEST.eq(digest))"));
}

#[test]
fn output_is_deterministic() {
    let build = || render_alone(&table(vec![column("id", "i64")], &["id"]));
    assert_eq!(build(), build());
}

/// A unique index on `lower(email)` makes `by_email`, which compares
/// ignoring case and, on a soft-delete table, sees only live rows.
#[test]
fn case_insensitive_unique_keys_look_up_ignoring_case() {
    let mut users = table(
        vec![
            column("id", "i64"),
            column("email", "String"),
            column("deleted_at", "Option<chrono::DateTime<chrono::Utc>>"),
        ],
        &["id"],
    );
    users.soft_delete_column = Some("deleted_at".to_owned());
    users.unique_keys.push(crate::schema::UniqueKey {
        columns: vec!["email".to_owned()],
        predicate: Some("deleted_at IS NULL".to_owned()),
        lowered: Some("email".to_owned()),
    });
    let rendered = render_alone(&users);
    assert!(
        rendered.contains("pub fn by_email(email: &str) -> rok_data::Select<User> {"),
        "{rendered}"
    );
    assert!(
        rendered.contains(r#"rok_data::Expr::raw("lower(email) = lower(?)", [email])"#),
        "{rendered}"
    );
}

/// A unique index on `(org, lower(name))` makes `by_org_and_name`,
/// comparing `org` as it is and `name` ignoring case.
#[test]
fn composite_keys_with_a_lowered_column_look_it_up_ignoring_case() {
    let mut tags = table(
        vec![
            column("id", "i64"),
            column("org", "i64"),
            column("name", "String"),
        ],
        &["id"],
    );
    tags.unique_keys.push(crate::schema::UniqueKey {
        columns: vec!["org".to_owned(), "name".to_owned()],
        predicate: None,
        lowered: Some("name".to_owned()),
    });
    let rendered = render_alone(&tags);
    assert!(rendered.contains("pub fn by_org_and_name("), "{rendered}");
    assert!(rendered.contains("ORG.eq(org)"), "{rendered}");
    assert!(
        rendered.contains(r#"rok_data::Expr::raw("lower(name) = lower(?)", [name])"#),
        "{rendered}"
    );
}
