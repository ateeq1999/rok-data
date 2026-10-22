//! The `Model` derive: the row type, its columns, and its relations.

use syn::{Ident, Path, spanned::Spanned};

pub(crate) mod conventions;
pub(crate) mod expand;
pub(crate) mod from_row;
#[cfg(test)]
mod tests;
pub(crate) struct Field {
    ident: Ident,
    column: String,
    primary_key: bool,
    generated: bool,
    skip: bool,
    created_at: bool,
    updated_at: bool,
    deleted_at: bool,
    version: bool,
    tenant: bool,
    belongs_to: Option<(Ident, Path)>,
    rules: Vec<Rule>,
}

/// A `#[rok(validate(…))]` rule.
enum Rule {
    Length(Option<syn::Expr>, Option<syn::Expr>),
    Range(Option<syn::Expr>, Option<syn::Expr>),
    Email,
    NonEmpty,
    Custom(Path),
}

fn parse_bounds(
    meta: &syn::meta::ParseNestedMeta<'_>,
) -> syn::Result<(Option<syn::Expr>, Option<syn::Expr>)> {
    let (mut min, mut max) = (None, None);
    meta.parse_nested_meta(|b| {
        if b.path.is_ident("min") {
            min = Some(b.value()?.parse()?);
        } else if b.path.is_ident("max") {
            max = Some(b.value()?.parse()?);
        } else {
            return Err(b.error("expected `min` or `max`"));
        }
        Ok(())
    })?;
    if min.is_none() && max.is_none() {
        return Err(meta.error("expected at least one of `min`, `max`"));
    }
    Ok((min, max))
}

fn parse_rules(meta: &syn::meta::ParseNestedMeta<'_>, rules: &mut Vec<Rule>) -> syn::Result<()> {
    meta.parse_nested_meta(|r| {
        if r.path.is_ident("length") {
            let (min, max) = parse_bounds(&r)?;
            rules.push(Rule::Length(min, max));
        } else if r.path.is_ident("range") {
            let (min, max) = parse_bounds(&r)?;
            rules.push(Rule::Range(min, max));
        } else if r.path.is_ident("email") {
            rules.push(Rule::Email);
        } else if r.path.is_ident("non_empty") {
            rules.push(Rule::NonEmpty);
        } else if r.path.is_ident("custom") {
            rules.push(Rule::Custom(r.value()?.parse()?));
        } else {
            return Err(r.error(
                "unknown rule; expected `length`, `range`, `email`, `non_empty` or `custom`",
            ));
        }
        Ok(())
    })
}

/// `has_many(name = Model::FK_COLUMN)` / `has_one(…)`.
struct Relation {
    kind: &'static str,
    name: Ident,
    model: Path,
    foreign_key: Path,
}

fn parse_relations(
    meta: &syn::meta::ParseNestedMeta<'_>,
    kind: &'static str,
    out: &mut Vec<Relation>,
) -> syn::Result<()> {
    meta.parse_nested_meta(|inner| {
        let name = inner
            .path
            .get_ident()
            .cloned()
            .ok_or_else(|| inner.error("expected `name = Model::FOREIGN_KEY`"))?;
        let foreign_key: Path = inner.value()?.parse()?;
        let mut model = foreign_key.clone();
        if model.segments.len() < 2 {
            return Err(syn::Error::new(
                foreign_key.span(),
                "expected a column constant such as `Post::USER_ID`",
            ));
        }
        model.segments.pop();
        model.segments.pop_punct();
        out.push(Relation {
            kind,
            name,
            model,
            foreign_key,
        });
        Ok(())
    })
}
