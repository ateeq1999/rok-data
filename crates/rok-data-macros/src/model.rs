//! The `Model` derive: the row type, its columns, and its relations.

use syn::{Ident, LitStr, Path, spanned::Spanned};

mod accessors;
pub(crate) mod conventions;
pub(crate) mod expand;
pub(crate) mod from_row;
pub(crate) mod input;
pub(crate) mod rules;
pub(crate) mod scopes;
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
    /// The field's type, for the setters the derive writes.
    ty: syn::Type,
    /// `set_with = path`: a `set_<field>` that writes `path(value)`.
    set_with: Option<Path>,
    /// `transform(…)`: what an input's field goes through before its rules.
    transforms: Vec<rules::Transform>,
    /// `nested`: the field holds input of its own, validated with this one.
    nested: bool,
    /// `when = path`: the field's rules run only when `path(self)` holds.
    when: Option<Path>,
}

/// A `#[rok(validate(…))]` rule.
enum Rule {
    Length(Option<syn::Expr>, Option<syn::Expr>),
    Range(Option<syn::Expr>, Option<syn::Expr>),
    Email,
    NonEmpty,
    Custom(Path),
    /// A database rule (input only): `unique(…)`, `exists(…)`, `database = path`.
    Database(rules::DatabaseRule),
    /// `required_if(field = other, equals = value)` / `required_unless(…)`:
    /// the field must be given when `self.other == value` holds (or doesn't).
    RequiredIf {
        other: Ident,
        equals: syn::Expr,
        negated: bool,
    },
    /// `required_with(other)`: the field must be given when `other` is.
    RequiredWith(Ident),
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
        } else if r.path.is_ident("required_if") || r.path.is_ident("required_unless") {
            let negated = r.path.is_ident("required_unless");
            let (mut other, mut equals) = (None, None);
            r.parse_nested_meta(|option| {
                if option.path.is_ident("field") {
                    other = Some(option.value()?.parse()?);
                } else if option.path.is_ident("equals") {
                    equals = Some(option.value()?.parse()?);
                } else {
                    return Err(option.error("expected `field = other_field` and `equals = value`"));
                }
                Ok(())
            })?;
            let (Some(other), Some(equals)) = (other, equals) else {
                return Err(r.error("expected `field = other_field` and `equals = value`"));
            };
            rules.push(Rule::RequiredIf { other, equals, negated });
        } else if r.path.is_ident("required_with") {
            let content;
            syn::parenthesized!(content in r.input);
            rules.push(Rule::RequiredWith(content.parse()?));
        } else if let Some(rule) = rules::parse_database_rule(&r)? {
            rules.push(Rule::Database(rule));
        } else {
            return Err(r.error(
                "unknown rule; expected `length`, `range`, `email`, `non_empty`, `custom`, `required_if`, `required_unless`, `required_with`, `unique`, `exists` or `database`",
            ));
        }
        Ok(())
    })
}

/// `has_many(name = Model::FK_COLUMN)` / `has_one(…)`, or a through
/// relation, `has_many_through(name = Model::FK_COLUMN, through =
/// Through::FK_COLUMN)` / `has_one_through(…)`.
struct Relation {
    kind: &'static str,
    name: Ident,
    model: Path,
    foreign_key: Path,
    /// The intermediate model's column holding this model's key, and the
    /// intermediate model.
    through: Option<(Path, Path)>,
    /// A many-to-many relation's pivot table and its two key columns
    /// (defaulted from the models' names when not given).
    pivot: Option<Pivot>,
}

/// `belongs_to_many(name = Target, pivot = "table", foreign_key = "…",
/// related_key = "…")`.
pub(crate) struct Pivot {
    pub(crate) table: LitStr,
    pub(crate) foreign_key: Option<LitStr>,
    pub(crate) related_key: Option<LitStr>,
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
            through: None,
            pivot: None,
        });
        Ok(())
    })
}

/// The model a column constant belongs to (`Post::USER_ID` → `Post`).
fn owner_of(column: &Path) -> syn::Result<Path> {
    let mut model = column.clone();
    if model.segments.len() < 2 {
        return Err(syn::Error::new(
            column.span(),
            "expected a column constant such as `Post::USER_ID`",
        ));
    }
    model.segments.pop();
    model.segments.pop_punct();
    Ok(model)
}

/// `has_many_through(name = Target::FK, through = Through::FK)`: one
/// relation per attribute.
fn parse_through(
    meta: &syn::meta::ParseNestedMeta<'_>,
    kind: &'static str,
    out: &mut Vec<Relation>,
) -> syn::Result<()> {
    let mut target: Option<(Ident, Path)> = None;
    let mut through: Option<Path> = None;
    meta.parse_nested_meta(|inner| {
        if inner.path.is_ident("through") {
            through = Some(inner.value()?.parse()?);
        } else {
            let name = inner.path.get_ident().cloned().ok_or_else(|| {
                inner.error("expected `name = Target::FOREIGN_KEY, through = Through::FOREIGN_KEY`")
            })?;
            target = Some((name, inner.value()?.parse()?));
        }
        Ok(())
    })?;
    let (Some((name, foreign_key)), Some(through)) = (target, through) else {
        return Err(
            meta.error("expected `name = Target::FOREIGN_KEY, through = Through::FOREIGN_KEY`")
        );
    };
    out.push(Relation {
        kind,
        name,
        model: owner_of(&foreign_key)?,
        foreign_key,
        through: Some((owner_of(&through)?, through)),
        pivot: None,
    });
    Ok(())
}

/// `belongs_to_many(roles = Role, pivot = "role_user")`, with optional
/// `foreign_key = "user_id"` / `related_key = "role_id"`: one relation per
/// attribute.
fn parse_many_to_many(
    meta: &syn::meta::ParseNestedMeta<'_>,
    out: &mut Vec<Relation>,
) -> syn::Result<()> {
    const EXPECTED: &str = "expected `name = Target, pivot = \"table\"` (and optionally `foreign_key = \"…\"`, `related_key = \"…\"`)";
    let mut target: Option<(Ident, Path)> = None;
    let mut table = None;
    let mut foreign_key = None;
    let mut related_key = None;
    meta.parse_nested_meta(|inner| {
        if inner.path.is_ident("pivot") {
            table = Some(inner.value()?.parse::<LitStr>()?);
        } else if inner.path.is_ident("foreign_key") {
            foreign_key = Some(inner.value()?.parse::<LitStr>()?);
        } else if inner.path.is_ident("related_key") {
            related_key = Some(inner.value()?.parse::<LitStr>()?);
        } else {
            let name = inner
                .path
                .get_ident()
                .cloned()
                .ok_or_else(|| inner.error(EXPECTED))?;
            target = Some((name, inner.value()?.parse()?));
        }
        Ok(())
    })?;
    let (Some((name, model)), Some(table)) = (target, table) else {
        return Err(meta.error(EXPECTED));
    };
    out.push(Relation {
        kind: "BelongsToMany",
        name,
        foreign_key: model.clone(),
        model,
        through: None,
        pivot: Some(Pivot {
            table,
            foreign_key,
            related_key,
        }),
    });
    Ok(())
}

impl Field {
    /// Whether the field carries an attribute only a model column has
    /// (a key, a managed timestamp, a relation, `set_with`).
    pub(crate) fn is_column_only(&self) -> bool {
        self.primary_key
            || self.generated
            || self.created_at
            || self.updated_at
            || self.deleted_at
            || self.version
            || self.tenant
            || self.belongs_to.is_some()
            || self.set_with.is_some()
    }
}
