//! Typed functions for annotated queries.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use sqlparser::keywords::Keyword;
use sqlparser::tokenizer::Token;

use crate::describe::Description;
use crate::emit::{Names, raw_string};
use crate::ir::{Query, QueryKind, Schema};

use returns::{Returns, returns};
use warnings::scope_warnings;

use params::{guess_name, insert_names, is_keyword, name_at, placeholder, words};

use crate::types::{self, Feature};
use crate::{Error, naming};

mod params;
mod returns;
mod warnings;

/// Facts read from the query text.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Shape {
    /// Name of each `$n` parameter (index 0 is `$1`).
    pub(crate) param_names: Vec<String>,
    /// Tables written by `INSERT INTO`, `UPDATE` and `DELETE FROM`.
    pub(crate) writes: Vec<String>,
    /// Tables named after `FROM`, `JOIN`, `INTO` and `UPDATE`.
    pub(crate) reads: Vec<String>,
}

pub(crate) fn shape(query: &Query) -> Result<Shape, Error> {
    let tokens = words(&query.sql).map_err(|e| Error::at(&query.location, e))?;
    let count = tokens.iter().filter_map(placeholder).max().unwrap_or(0);
    let mut names: Vec<Option<String>> = vec![None; count];
    let mut shape = Shape::default();

    let mut i = 0;
    while i < tokens.len() {
        let t = &tokens[i];
        if let Some(n) = placeholder(t) {
            let guess = guess_name(&tokens, i);
            if names[n - 1].is_none() {
                names[n - 1] = guess;
            }
        }
        if let Token::Word(w) = t {
            let target = match w.keyword {
                Keyword::INTO | Keyword::UPDATE => Some(true),
                Keyword::FROM => Some(is_keyword(tokens.get(i.wrapping_sub(1)), Keyword::DELETE)),
                Keyword::JOIN => Some(false),
                _ => None,
            };
            if let Some(writes) = target {
                let mut j = i + 1;
                if is_keyword(tokens.get(j), Keyword::ONLY) {
                    j += 1;
                }
                if let Some((name, next)) = name_at(&tokens, j) {
                    // `ON CONFLICT ... DO UPDATE SET` is not a table.
                    let is_set = is_keyword(tokens.get(j), Keyword::SET);
                    if !is_set {
                        if writes && !shape.writes.contains(&name) {
                            shape.writes.push(name.clone());
                        }
                        if !shape.reads.contains(&name) {
                            shape.reads.push(name.clone());
                        }
                    }
                    if w.keyword == Keyword::INTO {
                        insert_names(&tokens, next, &mut names);
                    }
                }
            }
        }
        i += 1;
    }
    for (n, name) in &query.param_names {
        if *n == 0 || *n > count {
            return Err(Error::at(
                &query.location,
                format!("`-- param: ${n}` but the query has {count} parameter(s)"),
            ));
        }
        names[n - 1] = Some(name.clone());
    }
    let mut used = BTreeSet::new();
    for (n, name) in names.into_iter().enumerate() {
        let mut name = naming::snake_ident(&name.unwrap_or_else(|| format!("p{}", n + 1)));
        if name == "db" || !used.insert(name.clone()) {
            name = format!("{name}_{}", n + 1);
            used.insert(name.clone());
        }
        shape.param_names.push(name);
    }
    Ok(shape)
}

/// Generate the functions of one module. Warnings (missing tenant or
/// soft-delete filters) are appended to `warnings`.
pub(crate) fn functions(
    queries: &[(&Query, &Description)],
    schema: &Schema,
    features: &mut BTreeSet<Feature>,
    warnings: &mut Vec<String>,
) -> Result<String, Error> {
    let names = Names::new(schema);
    let mut code = String::new();
    for (query, description) in queries {
        let at = |m: String| Error::at(&query.location, m);
        if query.sql.contains('?') {
            return Err(at(
                "`?` can't be used in a generated query (rok-data would read it as a placeholder); \
                 use `$n` parameters and functions such as `jsonb_exists` instead of the `?` operator"
                    .to_owned(),
            ));
        }
        let shape = shape(query)?;
        if shape.param_names.len() != description.params.len() {
            return Err(at(format!(
                "PostgreSQL reports {} parameter(s), the query text has {}",
                description.params.len(),
                shape.param_names.len()
            )));
        }
        scope_warnings(query, &shape, schema, warnings);

        let mut params = Vec::new();
        for (name, pg) in shape.param_names.iter().zip(&description.params) {
            let canonical = types::canonical_from_pg(pg);
            let ty = names
                .rust_type(&canonical, false, features)
                .map_err(|e| at(format!("parameter `{name}`: {e}")))?;
            let arg = match ty.as_str() {
                "String" => "&str".to_owned(),
                t if t.starts_with("Vec<") => format!("&[{}]", &t[4..t.len() - 1]),
                t => t.to_owned(),
            };
            params.push((name.clone(), arg));
        }

        let returns = if query.kind == QueryKind::Exec {
            Returns::Nothing
        } else {
            returns(query, description, &shape, schema, &names, features).map_err(at)?
        };

        let fn_name = &query.name;
        let row_type = match &returns {
            Returns::Model(path) | Returns::Scalar(path) => path.clone(),
            Returns::Row(name, fields) => {
                let _ = writeln!(code, "/// A row returned by [`{fn_name}`].");
                let _ = writeln!(
                    code,
                    "#[derive(Debug, Clone, PartialEq, rok_data::FromRow)]\npub struct {name} {{"
                );
                for (field, column, ty) in fields {
                    let _ = writeln!(code, "    /// The `{column}` column.");
                    if field != column {
                        let _ = writeln!(code, "    #[rok(column = {column:?})]");
                    }
                    let _ = writeln!(code, "    pub {field}: {ty},");
                }
                code.push_str("}\n\n");
                name.clone()
            }
            Returns::Nothing => String::new(),
        };
        let scalar = matches!(returns, Returns::Scalar(_));
        let decode = if scalar {
            format!("({row_type},)")
        } else {
            row_type.clone()
        };

        for line in &query.doc {
            let _ = writeln!(code, "/// {line}");
        }
        if !query.doc.is_empty() {
            code.push_str("///\n");
        }
        let kind = match query.kind {
            QueryKind::One => ":one",
            QueryKind::OneRequired => ":one!",
            QueryKind::Many => ":many",
            QueryKind::Stream => ":stream",
            QueryKind::Exec => ":exec",
        };
        let _ = writeln!(
            code,
            "/// `-- name: {fn_name} {kind}` from `{}`:\n///\n/// ```sql",
            query.location
        );
        for line in query.sql.lines() {
            let _ = writeln!(code, "/// {}", line.trim_end());
        }
        code.push_str("/// ```\n");

        let args: String = params.iter().map(|(n, t)| format!(", {n}: {t}")).collect();
        let mut chain = format!("rok_data::raw({})", raw_string(&query.sql));
        for (name, _) in &params {
            let _ = write!(chain, ".bind({name})");
        }
        for table in &shape.writes {
            if schema.tables.iter().any(|t| &t.name == table) {
                let _ = write!(chain, ".invalidates({table:?})");
            }
        }
        let (ret, body) = match query.kind {
            QueryKind::Exec => (
                "rok_data::Result<u64>".to_owned(),
                format!("{chain}.execute(db).await"),
            ),
            QueryKind::One => (
                format!("rok_data::Result<Option<{row_type}>>"),
                if scalar {
                    format!(
                        "{chain}.fetch_optional::<{decode}, _>(db).await.map(|row| row.map(|(value,)| value))"
                    )
                } else {
                    format!("{chain}.fetch_optional(db).await")
                },
            ),
            QueryKind::OneRequired => (
                format!("rok_data::Result<{row_type}>"),
                if scalar {
                    format!("{chain}.fetch_one::<{decode}, _>(db).await.map(|(value,)| value)")
                } else {
                    format!("{chain}.fetch_one(db).await")
                },
            ),
            QueryKind::Many => (
                format!("rok_data::Result<Vec<{row_type}>>"),
                if scalar {
                    format!(
                        "{chain}.fetch_all::<{decode}, _>(db).await.map(|rows| rows.into_iter().map(|(value,)| value).collect())"
                    )
                } else {
                    format!("{chain}.fetch_all(db).await")
                },
            ),
            QueryKind::Stream => (
                format!("rok_data::BoxStream<'e, rok_data::Result<{row_type}>>"),
                if scalar {
                    format!(
                        "rok_data::StreamExt::boxed(rok_data::StreamExt::map({chain}.stream::<{decode}, _>(db), |row| row.map(|(value,)| value)))"
                    )
                } else {
                    format!("{chain}.stream(db)")
                },
            ),
        };
        if query.kind == QueryKind::Stream {
            let _ = writeln!(
                code,
                "pub fn {fn_name}<'e>(db: impl rok_data::Executor<'e> + 'e{args}) -> {ret} {{\n    {body}\n}}\n"
            );
        } else {
            let _ = writeln!(
                code,
                "pub async fn {fn_name}<'e>(db: impl rok_data::Executor<'e>{args}) -> {ret} {{\n    {body}\n}}\n"
            );
        }
    }
    Ok(code)
}

#[cfg(test)]
mod tests;
