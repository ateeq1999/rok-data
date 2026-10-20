//! The naming helpers the derives share.

use super::naming::{pluralize, snake_case};

#[test]
fn table_names() {
    let t = |s: &str| pluralize(&snake_case(s));
    assert_eq!(t("User"), "users");
    assert_eq!(t("BlogPost"), "blog_posts");
    assert_eq!(t("Category"), "categories");
    assert_eq!(t("Day"), "days");
    assert_eq!(t("Address"), "addresses");
    assert_eq!(t("HTTPRequest"), "http_requests");
    assert_eq!(t("Box"), "boxes");
}
