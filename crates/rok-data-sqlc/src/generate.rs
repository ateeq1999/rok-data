//! The generation pipeline behind [`generate`](crate::generate): read the
//! inputs, parse and analyze them, plan the files, and write them.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use heck::ToSnakeCase;

use crate::config::{Config, Derive};
use crate::output::is_generated_file;
use crate::queries::AnalyzedQuery;
use crate::report::{Error, RefusedFile, Report, Result, TableFailure, display_path};
use crate::schema::{self, BaseType, EnumType, Shape, Table};
use crate::{api_config, generator, output, parser};

mod query_files;
mod snapshot;

use query_files::{AnalyzedFile, analyze_query_file, query_inputs};
use snapshot::{prepared, sql as snapshot_sql};

pub(crate) fn generate(config: &Config) -> Result<Report> {
    analyze(config).map(|(report, _)| report)
}

/// What a run analyzed, for `check --db` to compare against a database:
/// every query, where it is written, and the tables and enums it was
/// analyzed against.
#[derive(Debug, Default)]
pub(crate) struct Analysis {
    pub(crate) queries: Vec<LocatedQuery>,
    /// Every table of the schema, typed ids filled in.
    pub(crate) tables: Vec<Table>,
    pub(crate) enums: Vec<EnumType>,
}

/// An analyzed query and the line of the file its `-- name:` is on.
#[derive(Debug)]
pub(crate) struct LocatedQuery {
    pub(crate) path: PathBuf,
    pub(crate) line: usize,
    pub(crate) query: AnalyzedQuery,
}

/// A [`generate`] run that also answers its [`Analysis`].
pub(crate) fn analyze(config: &Config) -> Result<(Report, Analysis)> {
    let mut report = Report::default();
    let mut analysis = Analysis::default();
    match run(config, &mut report, &mut analysis) {
        Ok(()) => Ok((report, analysis)),
        Err(err) => Err(Error::new(err, report)),
    }
}

fn run(config: &Config, report: &mut Report, analysis: &mut Analysis) -> anyhow::Result<()> {
    let output_dir = &config.output;
    let resource = config.resource.as_deref();
    let (dry_run, force, lenient) = (config.dry_run, config.force, config.lenient);

    check_options(config)?;
    report.warnings.extend(scaffolded_document_warning(config));
    for (key, module) in &config.modules {
        if parser::naming::rust_identifier(module).as_deref() != Ok(module.as_str()) {
            bail!(
                "{} in the configuration renames the module to `{module}`, which is not a snake_case Rust identifier",
                module_location(key)
            );
        }
    }
    let sources = read_schema(config)?;
    // The undos a truth carries are the reverse half of pairs, not schema
    // (engine ADR 0017): [`snapshot::prepared`] runs the run-level strip
    // whose output feeds both this parse (the statement path strips anew
    // per source for its own reading, `parser/postgres/statements.rs`)
    // and the snapshot SQL below — where an undo SQL would undo the
    // change the truth means to have.
    let sources = prepared(sources)?;
    let mut parsed = parser::parse_sources_with_modules(
        &sources
            .iter()
            .map(|(path, sql)| parser::SchemaSource { path, sql })
            .collect::<Vec<_>>(),
        &config.modules,
    )
    .context("failed to parse SQL schema")?;
    report.warnings.append(&mut parsed.warnings);
    if let Some(key) = parsed.unmatched_modules.first() {
        bail!(
            "{} in the configuration names no table of the schema",
            module_location(key)
        );
    }
    parsed.fail_tables(|table| generator::table_failures(table, config));
    let sql = snapshot_sql(&sources);

    let matches_resource =
        |table: &str| resource.is_none_or(|name| table == name || table.to_snake_case() == name);

    report.table_failures = parsed
        .failures
        .iter()
        .filter(|failure| matches_resource(&failure.table))
        .map(|failure| TableFailure {
            table: failure.table.clone(),
            reason: failure.reason.clone(),
            path: failure.path.clone(),
            line: failure.line,
        })
        .collect();

    if !report.table_failures.is_empty() && !lenient {
        bail!(
            "{} unsupported table(s) found; pass --lenient to skip them",
            report.table_failures.len()
        );
    }

    let mut include: HashSet<String> = parsed
        .schema
        .tables
        .iter()
        .filter(|table| {
            resource.is_none_or(|name| table.sql_name == name || table.module_name == name)
        })
        .map(|table| table.module_name.clone())
        .collect();

    if let Ok(entries) = std::fs::read_dir(output_dir) {
        for entry in entries.flatten() {
            let module = entry.file_name().to_string_lossy().into_owned();
            let already_generated = entry.path().join("models.rs").is_file();
            if already_generated
                && parsed
                    .schema
                    .tables
                    .iter()
                    .any(|table| table.module_name == module)
            {
                include.insert(module);
            }
        }
    }

    let selected: Vec<_> = parsed
        .schema
        .tables
        .iter()
        .filter(|table| include.contains(&table.module_name))
        .cloned()
        .collect();

    if selected.is_empty() && report.table_failures.is_empty() {
        bail!("no matching resource found for {resource:?}");
    }
    if selected.is_empty() {
        if resource.is_some() {
            bail!("no generatable table matches {resource:?}");
        }
        bail!("all tables are unsupported; see warnings above");
    }

    generator::conventions::check(&selected, &config.conventions).map_err(anyhow::Error::msg)?;
    let kept = kept_resources(
        output_dir,
        &parsed.failures,
        &selected,
        &parsed.schema.enums,
    );
    // `[relationships]` is checked against the whole schema, kept tables
    // included, so a run limited to some tables (`--resource`) accepts the
    // same configuration.
    let schema_tables: Vec<Table> = parsed
        .schema
        .tables
        .iter()
        .chain(&kept.tables)
        .cloned()
        .collect();
    generator::relationships::check_changes(
        &schema_tables,
        &config.conventions,
        &config.relationships,
    )
    .map_err(anyhow::Error::msg)?;
    // `[generate.scopes]` likewise.
    generator::scopes::check(&schema_tables, &config.scopes).map_err(anyhow::Error::msg)?;
    // `[api]` likewise.
    if let Some(api) = &config.api {
        api_config::check(api, &schema_tables, &config.conventions).map_err(anyhow::Error::msg)?;
    }

    let query_inputs = query_inputs(&config.queries)?;
    if !query_inputs.is_empty() && selected.iter().any(|table| table.module_name == "queries") {
        bail!(
            "table module `queries` collides with the generated `queries` module; rename the table or omit --queries"
        );
    }
    analysis.tables = generator::keys::resolve(&parsed.schema.tables);
    analysis.enums.clone_from(&parsed.schema.enums);
    let mut analyzed = Vec::new();
    let mut failed_files = Vec::new();
    for input in &query_inputs {
        let AnalyzedFile {
            queries,
            failures,
            lines,
        } = analyze_query_file(&input.path, &parsed, &selected)?;
        analysis
            .queries
            .extend(queries.iter().map(|query| LocatedQuery {
                path: input.path.clone(),
                line: lines.get(&query.name).copied().unwrap_or(0),
                query: query.clone(),
            }));
        if !failures.is_empty() {
            failed_files.push(format!("`{}`", display_path(&input.path)));
        }
        report.query_failures.extend(failures);
        analyzed.push(generator::QueryModule {
            name: input.module.clone().unwrap_or_default(),
            queries,
        });
    }
    if !report.query_failures.is_empty() && !lenient {
        let count = report.query_failures.len();
        bail!(
            "{count} unsupported quer{} in {}; pass --lenient to skip them",
            if count == 1 { "y" } else { "ies" },
            failed_files.join(", ")
        );
    }
    let queries_module = match analyzed.as_slice() {
        [single] if query_inputs[0].module.is_none() => {
            generator::QueriesModule::Generate(&single.queries)
        }
        [] if is_generated_file(&output_dir.join("queries.rs"))
            || is_generated_file(&output_dir.join("queries").join("mod.rs")) =>
        {
            generator::QueriesModule::Keep
        }
        [] => generator::QueriesModule::None,
        modules => generator::QueriesModule::GenerateModules(modules),
    };

    let mut files = generator::plan_files_with_queries(
        config,
        &sql,
        &selected,
        &parsed.schema.enums,
        queries_module,
        &kept,
    )
    .map_err(anyhow::Error::msg)?;
    // The schema artifact (engine ADR 0010): the parsed schema in the codegen
    // shape, so the two engines hold and check one round-trip truth.
    files.push((
        output_dir.join("schema.json"),
        schema::snapshot::json(&parsed.schema)?,
    ));

    // Any generated file under the output directory that this run does not
    // produce is stale. A kept queries module protects both layouts
    // (`queries.rs` and everything under `queries/`); a run with queries
    // leaves the other layout's files stale, so switching layouts cleans up.
    // A kept resource directory is protected as a whole.
    let keep: HashSet<PathBuf> = files.iter().map(|(path, _)| path.clone()).collect();
    let mut stale = output::find_stale(output_dir, &keep)?;
    if matches!(queries_module, generator::QueriesModule::Keep) {
        let (single, dir) = (output_dir.join("queries.rs"), output_dir.join("queries"));
        stale.retain(|path| *path != single && !path.starts_with(&dir));
    }
    for table in &kept.tables {
        let dir = output_dir.join(&table.module_name);
        stale.retain(|path| !path.starts_with(&dir));
    }
    // `api.output` and `shared/` hold generated files beside the user's
    // own: a generated one this run doesn't produce is stale too. Scaffolded
    // files lack the generated marker, so they never are.
    let mut stale_roots = vec![(output_dir.clone(), stale)];
    if let Some(api) = &config.api {
        for root in generator::api_owned_dirs(api) {
            let found: Vec<PathBuf> = output::find_stale(&root, &keep)?
                .into_iter()
                .filter(|path| !stale_roots.iter().any(|(_, seen)| seen.contains(path)))
                .collect();
            stale_roots.push((root, found));
        }
    }
    let stale: Vec<PathBuf> = stale_roots
        .iter()
        .flat_map(|(_, paths)| paths.iter().cloned())
        .collect();

    if config.check {
        (report.missing, report.changed) = output::compare_files(&files)?;
        report.stale = stale;
        if report.has_drift() {
            let count = report.missing.len() + report.changed.len() + report.stale.len();
            bail!(
                "generated code is out of date: {count} file(s) drifted; run `rok generate` to update them"
            );
        }
        report.unchanged = files.into_iter().map(|(path, _)| path).collect();
        return Ok(());
    }

    let applied = output::apply_files(&files, dry_run, force)?;

    report.unchanged = applied.unchanged;
    report.refused = applied
        .refused
        .into_iter()
        .map(|(path, reason)| RefusedFile { path, reason })
        .collect();
    if !report.refused.is_empty() && !dry_run {
        bail!(
            "nothing was written; {} file(s) refused (use --force to overwrite)",
            report.refused.len()
        );
    }
    report.written = applied.written;
    report.scaffolded = applied.scaffolded;
    if !dry_run {
        for (root, paths) in &stale_roots {
            output::remove_stale(root, paths)?;
        }
    }
    report.stale = stale;
    Ok(())
}

/// The warning of a run with `[api] openapi = false` whose project still
/// has the `src/openapi.rs` `init` scaffolds (beside `api.output`), using
/// `GeneratedApiDoc`: a scaffolded file is never rewritten, so the project
/// no longer builds until the user removes the document's wiring.
fn scaffolded_document_warning(config: &Config) -> Option<String> {
    let api = config.api.as_ref().filter(|api| !api.openapi)?;
    let path = api.output.parent()?.join("openapi.rs");
    let text = std::fs::read_to_string(&path).ok()?;
    text.contains("GeneratedApiDoc").then(|| {
        format!(
            "`[api] openapi = false`, but {} uses `GeneratedApiDoc`, which is no longer generated: delete it, `pub mod openapi;` in `lib.rs` and the OpenAPI lines of `app.rs` (see \"Turning the document off\" in docs/guide/openapi.md)",
            display_path(&path)
        )
    })
}

/// Fails a run whose options need ones it doesn't set (engine ADRs 0015, 0016):
/// `validate` needs `serde`, extractors need `validate`, and the API needs
/// both derives and the extractor of the framework it is generated for.
fn check_options(config: &Config) -> anyhow::Result<()> {
    let validate = config.derive.contains(&Derive::Validate);
    if validate && !config.derive.contains(&Derive::Serde) {
        bail!(
            "`validate` in `derive` needs `serde`, since validated inputs and patches are deserialized: add `serde` to `derive`"
        );
    }
    if !config.extractors.is_empty() && !validate {
        bail!(
            "`extractors` needs `validate` in `derive`, since the extractors validate the bodies they take: add `serde` and `validate` to `derive`"
        );
    }
    let Some(api) = &config.api else {
        return Ok(());
    };
    let framework = api.framework;
    if !validate || !config.extractors.contains(&framework.extractor()) {
        bail!(
            "`api.output` needs `derive = [\"serde\", \"validate\"]` and `{}` in `extractors`, since API features take validated bodies through the {}-web extractor",
            framework.extractor().name(),
            framework.name().replace('_', "-"),
        );
    }
    Ok(())
}

/// The resource directories an earlier run generated for tables the schema
/// still defines but this run skips as unsupported (`--lenient`, or a table
/// outside `--resource`), with the tables' last good shapes. They are kept as
/// they are rather than removed as stale, and the run plans the other files
/// against those shapes, so the kept files still compile (engine ADR 0001).
///
/// A table the schema no longer defines (dropped, or never created) has no
/// failure, so its directory stays stale. So does one with no last good
/// shape (it failed at its `CREATE TABLE`, or its value lists cannot be
/// generated), or whose last good shape uses an enum that no longer
/// generates: the files kept for it could not compile.
fn kept_resources(
    output_dir: &Path,
    failures: &[parser::TableFailure],
    selected: &[Table],
    enums: &[EnumType],
) -> generator::KeptResources {
    let mut kept = generator::KeptResources::default();
    for last_good in failures
        .iter()
        .filter_map(|failure| failure.last_good.as_ref())
    {
        let table = &last_good.table;
        let module = &table.module_name;
        let taken = selected
            .iter()
            .chain(&kept.tables)
            .any(|other| other.module_name == *module);
        if taken || !is_generated_file(&output_dir.join(module).join("models.rs")) {
            continue;
        }
        let defined = |path: &str| {
            enums
                .iter()
                .chain(&last_good.enums)
                .any(|enum_type| enum_type.rust_path() == path)
        };
        let enums_generate = table.columns.iter().all(|column| {
            let value = match Shape::of(&column.sqlx_type) {
                Shape::List(item) => item,
                Shape::Optional(_) | Shape::Value(_) => column.sqlx_type.as_str(),
            };
            BaseType::of(value) != BaseType::Enum || defined(value)
        });
        if enums_generate {
            kept.tables.push(table.clone());
            kept.enums.extend(last_good.enums.iter().cloned());
        }
    }
    kept
}

/// Reads the schema inputs in order: a file as is, a directory as its
/// `*.sql` files in filename order without `*.down.sql`. Returns each file's
/// path and SQL, with line endings normalized to `\n` so the schema
/// snapshot is the same whichever way the files were checked out.
fn read_schema(config: &Config) -> anyhow::Result<Vec<(PathBuf, String)>> {
    if config.schema.is_empty() {
        bail!("no schema input given");
    }
    let mut files = Vec::new();
    for input in &config.schema {
        if input.is_dir() {
            let migrations = migration_files(input)?;
            if migrations.is_empty() {
                bail!(
                    "schema directory `{}` contains no `*.sql` migrations",
                    display_path(input)
                );
            }
            files.extend(migrations);
        } else {
            files.push(input.clone());
        }
    }
    files
        .into_iter()
        .map(|path| {
            let sql = read_text(&path)
                .with_context(|| format!("failed to read schema `{}`", display_path(&path)))?;
            Ok((path, sql))
        })
        .collect()
}

/// Reads a text input with CRLF line endings normalized to `\n`.
fn read_text(path: &Path) -> std::io::Result<String> {
    let text = std::fs::read_to_string(path)?;
    Ok(if text.contains("\r\n") {
        text.replace("\r\n", "\n")
    } else {
        text
    })
}

/// The up migrations in `dir` (not its subdirectories), in filename order.
fn migration_files(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let read_error = || format!("failed to read schema directory `{}`", display_path(dir));
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).with_context(read_error)? {
        let entry = entry.with_context(read_error)?;
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if name.ends_with(".sql") && !name.ends_with(".down.sql") && entry.path().is_file() {
            files.push(entry.path());
        }
    }
    files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    Ok(files)
}

/// Where a `[modules]` entry lives, as a dotted key path:
/// `modules."audit.users"`.
fn module_location(key: &str) -> String {
    if key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        format!("`modules.{key}`")
    } else {
        format!("`modules.\"{key}\"`")
    }
}
