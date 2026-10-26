use super::super::test_support::{column, table};
use super::*;

/// Renders `table` as if generated on its own.
fn render_alone(table: &Table) -> String {
    render(table, &[])
}

#[test]
fn renders_get_by_id_with_explicit_columns() {
    let rendered = render_alone(&table(
        vec![column("id", "i64"), column("email", "String")],
        &["id"],
    ));
    assert!(rendered.contains(
            "pub async fn get_by_id<'e, E>(executor: E, id: i64) -> Result<Option<User>, rok_data::Error>\nwhere\n    E: rok_data::Executor<'e>,\n{\n    User::filter(User::ID.eq(id)).first(executor).await\n}\n"
        ));
}

#[test]
fn renders_exists_list_page_and_count() {
    let rendered = render_alone(&table(
        vec![column("id", "i64"), column("email", "String")],
        &["id"],
    ));
    assert!(rendered.contains(
        "pub async fn exists_by_id<'e, E>(executor: E, id: i64) -> Result<bool, rok_data::Error>"
    ));
    assert!(rendered.contains(
        "    User::filter(User::ID.eq(id))\n        .count(executor)\n        .await\n        .map(|count| count > 0)\n"
    ));
    assert!(rendered.contains("    User::query().order_by(User::ID).all(executor).await\n"));
    assert!(rendered.contains(
        "    User::query()\n        .order_by(User::ID)\n        .limit(limit as u64)\n        .offset(offset as u64)\n        .all(executor)\n        .await\n"
    ));
    assert!(rendered.contains("    User::query().count(executor).await\n"));
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
    assert!(rendered.contains(
            "pub async fn get_by_id<'e, E>(executor: E, key: UserKey) -> Result<Option<User>, rok_data::Error>"
        ));
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
    assert!(rendered.contains("filter(User::ORDER_ID.eq(key.order_id))"));
    assert!(rendered.contains("filter(User::PRODUCT_ID.eq(key.product_id))"));
    assert!(rendered.contains(".order_by(User::ORDER_ID)"));
    assert!(rendered.contains(".order_by(User::PRODUCT_ID)"));
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
    // mixed-case SQL name is quoted in the row's model derive.
    assert!(rendered.contains("filter(User::SELECT.eq(select))"));
    assert!(rendered.contains("FROM \"Auth\".users"));
}

#[test]
fn key_params_avoid_local_name_collisions() {
    let rendered = render_alone(&table(vec![column("limit", "i64")], &["limit"]));
    assert!(rendered.contains("get_by_id<'e, E>(executor: E, limit_key: i64)"));
    assert!(rendered.contains("filter(User::LIMIT.eq(limit_key))"));
}

#[test]
fn without_primary_key_only_unordered_list_and_count() {
    let rendered = render_alone(&table(vec![column("email", "String")], &[]));
    assert!(!rendered.contains("pub async fn get_by_id"));
    assert!(!rendered.contains("pub async fn exists_by_id"));
    assert!(!rendered.contains("pub async fn list_page"));
    assert!(!rendered.contains("pub async fn search_after"));
    let list = rendered.split("pub async fn list<").nth(1).unwrap();
    let list = list.split("pub async fn").next().unwrap();
    assert!(
        !list.contains(".order_by("),
        "list has no stable order to offer"
    );
    assert!(
        !rendered.contains("query.push(\", ")
            && rendered.contains("query.push(sort.direction.sql());"),
        "search sorts by the caller's key with no primary-key tie-break"
    );
    assert!(rendered.contains("No primary key detected"));
    assert!(rendered.contains("pub async fn list<'e, E>(executor: E)"));
    assert!(rendered.contains("pub async fn count<'e, E>(executor: E)"));
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
    assert!(rendered.contains("get_by_id<'e, E>(executor: E, code: &UserId)"));
    assert!(rendered.contains("exists_by_id<'e, E>(executor: E, code: &UserId)"));
    assert!(rendered.contains("filter(User::CODE.eq(code))"));
    assert!(
        rendered.contains("    after: Option<&UserId>,\n"),
        "{rendered}"
    );

    let rendered = render_alone(&table(
        vec![column("order_id", "uuid::Uuid"), column("sku", "String")],
        &["order_id", "sku"],
    ));
    assert!(
        rendered.contains("(executor: E, key: &UserKey)"),
        "{rendered}"
    );
    assert!(rendered.contains("    after: Option<&UserKey>,\n"));
}

/// A `BYTEA` key is borrowed as a byte slice, like text as `&str`.
#[test]
fn byte_keys_are_borrowed_as_slices() {
    let rendered = render_alone(&table(
        vec![column("digest", "Vec<u8>"), column("email", "String")],
        &["digest"],
    ));
    assert!(rendered.contains("get_by_id<'e, E>(executor: E, digest: &[u8])"));
    assert!(
        rendered.contains("    after: Option<&[u8]>,\n"),
        "{rendered}"
    );
    assert!(rendered.contains("filter(User::DIGEST.eq(digest))"));
}

#[test]
fn output_is_deterministic() {
    let build = || render_alone(&table(vec![column("id", "i64")], &["id"]));
    assert_eq!(build(), build());
}

/// A unique index on `lower(email)` makes `get_by_email`, which compares
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
        rendered.contains("pub async fn get_by_email<'e, E>(executor: E, email: &str)"),
        "{rendered}"
    );
    assert!(
        rendered.contains(r#"rok_data::Expr::raw("lower(email) = lower(?)", [email])"#),
        "{rendered}"
    );
}

/// A unique index on `(org, lower(name))` makes `get_by_org_and_name`,
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
    assert!(rendered.contains("ORG.eq(org)"), "{rendered}");
    assert!(
        rendered.contains(r#"rok_data::Expr::raw("lower(name) = lower(?)", [name])"#),
        "{rendered}"
    );
}
