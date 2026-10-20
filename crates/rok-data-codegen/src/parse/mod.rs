//! Parsing the schema files into the generator's own model.

use crate::ir::{Query, QueryKind, Schema};
use crate::split::split;
use crate::{Error, naming};
use check::check;
use comments::{apply_comments, resolve_references};
use statements::schema_statement;

mod check;
mod comments;
mod statements;

pub(crate) use check::quote;

/// One input file.
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// Path for messages (`db/user.sql`).
    pub path: String,
    /// Module name (from the file stem).
    pub module: String,
    /// File contents.
    pub text: String,
}

/// The parsed input: the schema and the annotated queries.
#[derive(Debug, Default)]
pub struct Parsed {
    /// Declared schema.
    pub schema: Schema,
    /// Annotated queries.
    pub queries: Vec<Query>,
    /// Modules, in file order (shared `_` files excluded).
    pub modules: Vec<String>,
}

/// Parse every file. Files whose stem starts with `_` hold shared
/// statements; their enums land in the `types` module.
pub fn parse(files: &[SourceFile]) -> Result<Parsed, Error> {
    let mut parsed = Parsed::default();
    let mut comments: Vec<(String, Option<String>, Option<String>)> = Vec::new();
    for file in files {
        let shared = file.module.starts_with('_');
        let module = if shared {
            "types".to_owned()
        } else {
            file.module.clone()
        };
        if !shared && !parsed.modules.contains(&module) {
            parsed.modules.push(module.clone());
        }
        let statements = split(&file.text);
        let mut i = 0;
        while i < statements.len() {
            let stmt = &statements[i];
            let at = format!("{}:{}", file.path, stmt.line);
            let directives = Directives::read(&stmt.comments, &at)?;
            if directives.down {
                return Err(Error::at(
                    &at,
                    "`-- rok:down` must directly follow the statement it undoes",
                ));
            }
            // A `-- rok:down` statement right after this one is its undo SQL.
            let explicit_down = match statements.get(i + 1) {
                Some(next) if Directives::read(&next.comments, &at)?.down => {
                    i += 1;
                    Some(next.sql.clone())
                }
                _ => None,
            };
            if let Some((name, kind)) = directives.query {
                if shared {
                    return Err(Error::at(&at, "queries can't live in a shared `_` file"));
                }
                parsed.queries.push(Query {
                    name,
                    module: module.clone(),
                    kind,
                    sql: stmt.sql.clone(),
                    doc: directives.doc,
                    param_names: directives.params,
                    location: at,
                });
                i += 1;
                continue;
            }
            schema_statement(
                stmt,
                &module,
                directives.doc,
                explicit_down,
                &at,
                &mut parsed.schema,
                &mut comments,
            )?;
            i += 1;
        }
    }
    apply_comments(&mut parsed.schema, &comments);
    resolve_references(&mut parsed.schema);
    check(&parsed)?;
    Ok(parsed)
}

/// Directives and docs from a statement's comment block.
#[derive(Default)]
struct Directives {
    query: Option<(String, QueryKind)>,
    params: Vec<(usize, String)>,
    down: bool,
    doc: Vec<String>,
}

impl Directives {
    fn read(comments: &[String], at: &str) -> Result<Self, Error> {
        let mut d = Directives::default();
        for line in comments {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("name:") {
                let mut parts = rest.split_whitespace();
                let (Some(name), Some(kind), None) = (parts.next(), parts.next(), parts.next())
                else {
                    return Err(Error::at(
                        at,
                        "expected `-- name: <function> <:one|:one!|:many|:stream|:exec>`",
                    ));
                };
                let kind = QueryKind::parse(kind).ok_or_else(|| {
                    Error::at(
                        at,
                        format!(
                            "unknown query kind `{kind}`; use :one, :one!, :many, :stream or :exec"
                        ),
                    )
                })?;
                if naming::snake_ident(name) != name {
                    return Err(Error::at(
                        at,
                        format!("query name `{name}` must be a snake_case Rust identifier"),
                    ));
                }
                d.query = Some((name.to_owned(), kind));
            } else if let Some(rest) = line.strip_prefix("param:") {
                let mut parts = rest.split_whitespace();
                let parsed = match (parts.next(), parts.next()) {
                    (Some(p), Some(name)) => p
                        .strip_prefix('$')
                        .and_then(|n| n.parse::<usize>().ok())
                        .map(|n| (n, naming::snake_ident(name))),
                    _ => None,
                };
                d.params
                    .push(parsed.ok_or_else(|| Error::at(at, "expected `-- param: $<n> <name>`"))?);
            } else if line == "rok:down" {
                d.down = true;
            } else if let Some(directive) = line.strip_prefix("rok:") {
                return Err(Error::at(
                    at,
                    format!("unknown directive `rok:{directive}` here"),
                ));
            } else {
                d.doc.push(line.to_owned());
            }
        }
        Ok(d)
    }
}

#[cfg(test)]
mod tests;
