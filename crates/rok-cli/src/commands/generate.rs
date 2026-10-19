//! `rok generate` / `rok generate check`: the model-crate generator.
//!
//! Everything except option merging lives in the reusable generator library
//! (`rok-data-codegen::{generate, write, differences}`); this module only
//! turns flags into [`Options`](rok_data_codegen::Options) and formats the
//! report.

use std::process::ExitCode;

use rok_data_codegen::{Rename, differences, write};

use crate::cli::{CheckOptions, GenerateAction, GenerateOptions};
use crate::config;
use crate::context;

/// `rok generate`: with the flags directly (the original `rok-db-gen
/// generate <OPTIONS>` shape), or with the nested `rok generate check`
/// CI mode. A value given at the group level and again on the leaf: the
/// leaf wins; booleans OR.
pub(crate) async fn run(
    generation: GenerateOptions,
    action: Option<GenerateAction>,
) -> Result<ExitCode, String> {
    let GenerateOptions {
        config,
        input,
        output,
        crate_name,
        migration,
        renames,
        allow_destructive,
        strict,
    } = generation;
    let Some(GenerateAction::Check { options: leaf }) = action else {
        return generate(GenerateOptions {
            config,
            input,
            output,
            crate_name,
            migration,
            renames,
            allow_destructive,
            strict,
        })
        .await;
    };
    if migration.is_some() || !renames.is_empty() {
        // The original generator rejected these flags with `check`; don't
        // silently ignore them now that `check` nests under `generate`.
        return Err(
            "`--migration` and `--rename` belong to the generate run, not `check`: \
             pass them to `rok generate` without `check`"
                .to_owned(),
        );
    }
    let CheckOptions {
        config: leaf_config,
        input: leaf_input,
        output: leaf_output,
        crate_name: leaf_crate_name,
        allow_destructive: leaf_allow_destructive,
        strict: leaf_strict,
    } = leaf;
    check(CheckOptions {
        config: leaf_config.or(config),
        input: leaf_input.or(input),
        output: leaf_output.or(output),
        crate_name: leaf_crate_name.or(crate_name),
        allow_destructive: leaf_allow_destructive || allow_destructive,
        strict: leaf_strict || strict,
    })
    .await
}

/// `rok generate`.
pub(crate) async fn generate(options: GenerateOptions) -> Result<ExitCode, String> {
    let gen_options = codegen_options(
        options.config.as_deref(),
        options.input,
        options.output,
        options.crate_name.as_deref(),
        options.migration,
        options.renames,
        options.allow_destructive,
        options.strict,
    )?;
    let generated = rok_data_codegen::generate(&gen_options).map_err(|e| e.to_string())?;
    for warning in &generated.warnings {
        eprintln!("warning: {warning}");
    }
    let written = write(&gen_options, &generated).map_err(|e| e.to_string())?;
    if let Some(name) = &generated.new_migration {
        println!("added migration `{name}`");
    }
    if written.is_empty() {
        println!("{} is up to date", gen_options.output.display());
    } else {
        for path in written {
            println!("wrote {}", gen_options.output.join(path).display());
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// `rok generate check`.
pub(crate) async fn check(options: CheckOptions) -> Result<ExitCode, String> {
    let gen_options = codegen_options(
        options.config.as_deref(),
        options.input,
        options.output,
        options.crate_name.as_deref(),
        None,
        Vec::new(),
        options.allow_destructive,
        options.strict,
    )?;
    let generated = rok_data_codegen::generate(&gen_options).map_err(|e| e.to_string())?;
    for warning in &generated.warnings {
        eprintln!("warning: {warning}");
    }
    let stale = differences(&gen_options, &generated);
    if stale.is_empty() {
        println!("{} is up to date", gen_options.output.display());
        Ok(ExitCode::SUCCESS)
    } else {
        eprintln!("error: these generated files are out of date; run `rok generate`:");
        for path in stale {
            eprintln!("  {}", gen_options.output.join(path).display());
        }
        Ok(ExitCode::FAILURE)
    }
}

/// Parse `--rename` specs, resolve the environment database, and merge the
/// settings file.
// One parameter per flag on purpose: this is the single merge point between
// clap options and the settings file, and restructuring it into a struct
// would only move the noise around.
#[allow(clippy::too_many_arguments)]
fn codegen_options(
    config: Option<&std::path::Path>,
    input: Option<std::path::PathBuf>,
    output: Option<std::path::PathBuf>,
    crate_name: Option<&str>,
    migration: Option<String>,
    renames: Vec<String>,
    allow_destructive: bool,
    strict: bool,
) -> Result<rok_data_codegen::Options, String> {
    let renames = renames
        .iter()
        .map(|spec| {
            Rename::parse(spec)
                .ok_or_else(|| format!("--rename `{spec}`: expected `old=new` or `table.old=new`"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let database_url = context::env_database_url().filter(|d| !d.is_empty());
    config::options(
        config,
        input,
        output,
        crate_name,
        migration,
        renames,
        allow_destructive,
        strict,
        database_url,
    )
}
