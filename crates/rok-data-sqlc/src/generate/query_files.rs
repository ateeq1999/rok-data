//! The query files of a run: which files there are, the module each one
//! generates, and the analysis of its queries against the schema.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

use super::read_text;
use crate::generator::queries::SHARED;
use crate::queries::{AnalyzedQuery, QueryDef, ReturnMode};
use crate::report::{QueryFailure, SkippedQuery, display_path};
use crate::schema::Table;
use crate::{generator, parser, queries};

/// One query file to analyze.
pub(super) struct QueryInput {
    pub(super) path: PathBuf,
    /// The generated module under `queries/`, named after the file stem;
    /// `None` when a single query file becomes `queries.rs`.
    pub(super) module: Option<String>,
    /// Whether the file is in a query directory's `shared/`: its module is
    /// `queries::shared::<stem>`, and it may hold only reads.
    pub(super) shared: bool,
}

/// Expands the query inputs. A lone file keeps the single-module layout
/// (`queries.rs`); a directory (its `*.sql` files, sorted by name) or several
/// inputs produce one module per file under `queries/`. A directory's
/// `shared/` subdirectory produces one module per file under
/// `queries/shared/`.
pub(super) fn query_inputs(inputs: &[PathBuf]) -> anyhow::Result<Vec<QueryInput>> {
    if let [path] = inputs
        && !path.is_dir()
    {
        return Ok(vec![QueryInput {
            path: path.clone(),
            module: None,
            shared: false,
        }]);
    }

    let mut files = Vec::new();
    for input in inputs {
        if !input.is_dir() {
            files.push((input.clone(), false));
            continue;
        }
        let found = sql_files(input)?;
        let shared_dir = input.join(SHARED);
        let shared = if shared_dir.is_dir() {
            let shared = sql_files(&shared_dir)?;
            if shared.is_empty() {
                bail!(
                    "query directory `{}` contains no `.sql` files",
                    display_path(&shared_dir)
                );
            }
            shared
        } else {
            Vec::new()
        };
        if found.is_empty() && shared.is_empty() {
            bail!(
                "query directory `{}` contains no `.sql` files",
                display_path(input)
            );
        }
        files.extend(found.into_iter().map(|path| (path, false)));
        files.extend(shared.into_iter().map(|path| (path, true)));
    }

    // Names are unique within `queries` and within `queries::shared`.
    let mut seen: HashMap<(bool, String), PathBuf> = HashMap::new();
    let mut modules = Vec::with_capacity(files.len());
    for (path, shared) in files {
        let module = query_module_name(&path)?;
        if !shared && module == SHARED {
            bail!(
                "query file `{}`: module `{SHARED}` is reserved for the query directory's `{SHARED}/`, the reads any feature may call; rename the file, or move its reads to `{SHARED}/<name>.sql`",
                display_path(&path)
            );
        }
        if let Some(first) = seen.insert((shared, module.clone()), path.clone()) {
            let module = if shared {
                format!("{SHARED}::{module}")
            } else {
                module
            };
            bail!(
                "query files `{}` and `{}` would both generate module `{module}`; rename one of them",
                display_path(&first),
                display_path(&path)
            );
        }
        modules.push(QueryInput {
            path,
            module: Some(module),
            shared,
        });
    }
    Ok(modules)
}

/// The `*.sql` files directly in `dir`, sorted by name.
fn sql_files(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let read_error = || format!("failed to read query directory `{}`", display_path(dir));
    let mut found = std::fs::read_dir(dir)
        .with_context(read_error)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(read_error)?;
    found.retain(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "sql"));
    found.sort();
    Ok(found)
}

/// The module a query file generates under `queries/`: its stem, which must
/// already be a snake_case Rust identifier that is not a keyword.
fn query_module_name(path: &Path) -> anyhow::Result<String> {
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    match parser::naming::rust_identifier(&stem) {
        Ok(ident) if ident == stem => Ok(stem),
        suggestion => {
            let hint = match suggestion {
                Ok(ident) => format!("; rename it to `{ident}.sql`"),
                Err(_) => String::new(),
            };
            bail!(
                "query file `{}`: `{stem}` is not a valid Rust module name (use lowercase letters, digits and underscores, not starting with a digit or naming a keyword){hint}",
                display_path(path)
            )
        }
    }
}

/// One query file, analyzed.
pub(super) struct AnalyzedFile {
    pub(super) queries: Vec<AnalyzedQuery>,
    pub(super) failures: Vec<QueryFailure>,
    /// The queries marked `-- rok:skip`.
    pub(super) skipped: Vec<SkippedQuery>,
    /// Each query's `-- name:` line, by query name.
    pub(super) lines: HashMap<String, usize>,
}

/// Parses and analyzes one query file. Unsupported queries come back as failures for the caller to
/// report.
pub(super) fn analyze_query_file(
    path: &Path,
    shared: bool,
    parsed: &parser::ParsedSchema,
    selected: &[Table],
) -> anyhow::Result<AnalyzedFile> {
    let text = read_text(path)
        .with_context(|| format!("failed to read query file `{}`", display_path(path)))?;

    let (defs, mut failures) = queries::parse_query_file(&text);
    let lines = defs
        .iter()
        .map(|def| (def.name.clone(), def.line))
        .collect();
    if defs.is_empty() && failures.is_empty() {
        bail!(
            "query file `{}` contains no `-- name:` annotated queries",
            display_path(path)
        );
    }
    if shared {
        refuse_writes(path, &defs)?;
    }
    let generated_modules: HashSet<String> = selected
        .iter()
        .map(|table| table.module_name.clone())
        .collect();
    // Columns carry the typed ids the generated resources define, so query
    // parameters and result columns drawn from a key use them too.
    let mut tables = generator::keys::resolve(&parsed.schema.tables);
    for column in tables.iter_mut().flat_map(|table| &mut table.columns) {
        if column
            .typed_id
            .as_ref()
            .is_some_and(|id| !generated_modules.contains(&id.module))
        {
            column.typed_id = None;
        }
    }
    let (analyzed, analysis_failures) = queries::analyze_queries(
        &defs,
        &tables,
        &parsed.schema.enums,
        &parsed.schema.functions,
        &generated_modules,
        &parsed.failures,
    );
    failures.extend(analysis_failures);

    let failures = failures
        .into_iter()
        .map(|failure| QueryFailure {
            query: failure.query,
            reason: failure.reason,
            path: failure.line.map(|_| path.to_path_buf()),
            line: failure.line,
        })
        .collect();
    let skipped = defs
        .iter()
        .filter_map(|def| {
            def.skip.as_ref().map(|note| SkippedQuery {
                query: def.name.clone(),
                note: note.clone(),
                path: path.to_path_buf(),
                line: def.line,
            })
        })
        .collect();
    Ok(AnalyzedFile {
        queries: analyzed,
        failures,
        skipped,
        lines,
    })
}

/// Fails when a query in `shared/` (the file at `path`) writes: a write kind
/// (`:exec`, `:execrows`) or a statement that is not a read. Each is named
/// with its file, line and query.
fn refuse_writes(path: &Path, defs: &[QueryDef]) -> anyhow::Result<()> {
    let writes: Vec<String> = defs
        .iter()
        .filter_map(|def| {
            let what = match def.mode {
                ReturnMode::Exec | ReturnMode::ExecRows => {
                    format!("is `:{}`", def.mode.keyword())
                }
                ReturnMode::One | ReturnMode::MaybeOne | ReturnMode::Many => {
                    format!("is {}", write_statement(&def.sql)?)
                }
            };
            Some(format!(
                "{}:{}: query `{}` {what}",
                display_path(path),
                def.line,
                def.name
            ))
        })
        .collect();
    if writes.is_empty() {
        return Ok(());
    }
    bail!(
        "{}\nqueries in a query directory's `{SHARED}/` are reads any feature may call (`:one`, `:many`, `:maybe_one` over a SELECT); move a write to its feature's query file",
        writes.join("\n")
    )
}

/// The kind of statement ("an UPDATE"), when the SQL is an INSERT, UPDATE, DELETE or
/// MERGE. (A data-modifying CTE fails the analysis anyway.)
fn write_statement(sql: &str) -> Option<&'static str> {
    use sqlparser::dialect::PostgreSqlDialect;
    use sqlparser::keywords::Keyword;
    use sqlparser::tokenizer::{Token, Tokenizer};

    let tokens = Tokenizer::new(&PostgreSqlDialect {}, sql).tokenize().ok()?;
    let first = tokens
        .iter()
        .find(|token| !matches!(token, Token::Whitespace(_)))?;
    let Token::Word(word) = first else {
        return None;
    };
    match word.keyword {
        Keyword::INSERT => Some("an INSERT"),
        Keyword::UPDATE => Some("an UPDATE"),
        Keyword::DELETE => Some("a DELETE"),
        Keyword::MERGE => Some("a MERGE"),
        _ => None,
    }
}
