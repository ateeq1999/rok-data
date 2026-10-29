//! The query files of a run: which files there are, the module each one
//! generates, and the analysis of its queries against the schema.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

use super::read_text;
use crate::queries::AnalyzedQuery;
use crate::report::{QueryFailure, display_path};
use crate::schema::Table;
use crate::{generator, parser, queries};

/// One query file to analyze.
pub(super) struct QueryInput {
    pub(super) path: PathBuf,
    /// The generated module under `queries/`, named after the file stem;
    /// `None` when a single query file becomes `queries.rs`.
    pub(super) module: Option<String>,
}

/// Expands the query inputs. A lone file keeps the single-module layout
/// (`queries.rs`); a directory (its `*.sql` files, sorted by name) or several
/// inputs produce one module per file under `queries/`.
pub(super) fn query_inputs(inputs: &[PathBuf]) -> anyhow::Result<Vec<QueryInput>> {
    if let [path] = inputs
        && !path.is_dir()
    {
        return Ok(vec![QueryInput {
            path: path.clone(),
            module: None,
        }]);
    }

    let mut files = Vec::new();
    for input in inputs {
        if !input.is_dir() {
            files.push(input.clone());
            continue;
        }
        let read_error = || format!("failed to read query directory `{}`", display_path(input));
        let mut found = std::fs::read_dir(input)
            .with_context(read_error)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()
            .with_context(read_error)?;
        found.retain(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "sql"));
        if found.is_empty() {
            bail!(
                "query directory `{}` contains no `.sql` files",
                display_path(input)
            );
        }
        found.sort();
        files.extend(found);
    }

    let mut seen: HashMap<String, PathBuf> = HashMap::new();
    let mut modules = Vec::with_capacity(files.len());
    for path in files {
        let module = query_module_name(&path)?;
        if let Some(first) = seen.insert(module.clone(), path.clone()) {
            bail!(
                "query files `{}` and `{}` would both generate module `{module}`; rename one of them",
                display_path(&first),
                display_path(&path)
            );
        }
        modules.push(QueryInput {
            path,
            module: Some(module),
        });
    }
    Ok(modules)
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
    /// Each query's `-- name:` line, by query name.
    pub(super) lines: HashMap<String, usize>,
}

/// Parses and analyzes one query file. Unsupported queries come back as failures for the caller to
/// report.
pub(super) fn analyze_query_file(
    path: &Path,
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
    Ok(AnalyzedFile {
        queries: analyzed,
        failures,
        lines,
    })
}
