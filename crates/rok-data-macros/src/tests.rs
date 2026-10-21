//! The naming helpers the derives share.

use proc_macro2::Span;
use syn::Ident;

use super::naming::{method_ident, pluralize, relation_ident, snake_case};

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

/// A relation named after a Rust keyword becomes a raw identifier, so the
/// generated accessor compiles.
#[test]
fn a_keyword_relation_name_is_a_raw_identifier() {
    let ident = relation_ident("type", Span::call_site()).expect("`type` has a raw form");
    assert_eq!(ident.to_string(), "r#type");
    assert_eq!(
        method_ident(&ident)
            .expect("`type` has a raw form")
            .to_string(),
        "r#type"
    );
}

/// A relation named after a word that is no keyword stays as it is.
#[test]
fn a_plain_relation_name_stays_as_it_is() {
    let ident = relation_ident("posts", Span::call_site()).expect("`posts` is a plain name");
    assert_eq!(ident.to_string(), "posts");
    assert_eq!(
        method_ident(&ident)
            .expect("`posts` is a plain name")
            .to_string(),
        "posts"
    );
}

/// A keyword with no raw form is refused: `r#self` does not exist.
#[test]
fn a_keyword_without_a_raw_form_is_refused() {
    let error = relation_ident("self", Span::call_site())
        .expect_err("`self` has no raw form")
        .to_string();
    assert!(error.contains("self"), "{error}");

    let error = method_ident(&Ident::new("self", Span::call_site()))
        .expect_err("`self` has no raw form")
        .to_string();
    assert!(error.contains("self"), "{error}");
}
