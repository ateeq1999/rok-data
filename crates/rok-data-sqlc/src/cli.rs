//! The `rok-data-sqlc` command line: parses flags into a [`Config`], calls
//! [`generate`](crate::generate), and prints the [`Report`](crate::Report).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

mod args;
mod check;
mod generate;
mod print;
mod watch;

use args::{FrameworkArg, ModeArgs};
use generate::GenerateArgs;
use print::{print_next_steps, print_report, print_scaffolded};

use crate::config_file;
use crate::report::display_path;
use crate::{ApiFramework, Config};

#[derive(Debug, Parser)]
#[command(
    name = "rok-data-sqlc",
    version,
    about = "Generate SQLx Rust modules from SQL schema files"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
#[allow(
    clippy::large_enum_variant,
    reason = "parsed once per run; boxing the flags buys nothing"
)]
enum Command {
    /// Generate Rust database modules from a schema file
    #[command(visible_alias = "g")]
    Generate(GenerateArgs),

    /// Scaffold the actix-web application skeleton (engine ADR 0016) into a
    /// project, each file only if absent; generates nothing
    #[command(visible_alias = "i")]
    Init {
        /// The project directory
        #[arg(value_name = "DIR", default_value = ".")]
        dir: PathBuf,

        /// Copy this ready-made project instead of the skeleton, each file
        /// only if absent, leaving out its generated files
        #[arg(
            short,
            long,
            value_name = "NAME",
            value_parser = clap::builder::PossibleValuesParser::new(crate::init::template_names())
        )]
        template: Option<String>,

        /// The web framework the skeleton is written for: `actix_web` (the
        /// default) or `axum`. A template has its own, so this doesn't
        /// combine with `--template`
        #[arg(short = 'w', long, value_name = "FW", conflicts_with = "template")]
        api_framework: Option<FrameworkArg>,
    },

    /// Check the project against engine ADR 0016's architecture: the feature
    /// layout and rules 1, 2, 4, 5 and 6; print each violation as
    /// `path:line: rule: message` and exit 1 if any
    #[command(visible_alias = "a")]
    CheckArch {
        /// Report layout violations as warnings instead of failing
        #[arg(short, long)]
        lenient: bool,

        /// Read `api.output` and `api.db_path` from this config file instead
        /// of discovering rok-data-sqlc.toml in the current directory or its
        /// parents
        #[arg(short, long, value_name = "FILE")]
        config: Option<PathBuf>,
    },

    /// Check that the generated code is up to date and, with `--db`, that
    /// every annotated query prepares against the database with the types
    /// the generated code binds and decodes; exit 1 on any difference
    #[command(visible_alias = "c")]
    Check {
        /// Also prepare every annotated query, without running it, against
        /// this database [default: DATABASE_URL]
        #[arg(
            short,
            long,
            value_name = "URL",
            num_args = 0..=1,
            default_missing_value = ""
        )]
        db: Option<String>,

        /// Skip unsupported tables and queries with warnings instead of failing
        #[arg(short, long)]
        lenient: bool,

        /// Read options from this config file instead of discovering
        /// rok-data-sqlc.toml in the current directory or its parents
        #[arg(short, long, value_name = "FILE")]
        config: Option<PathBuf>,
    },

    /// Work with annotated queries
    #[command(visible_alias = "q")]
    Query {
        #[command(subcommand)]
        action: QueryCommand,
    },
}

#[derive(Debug, Subcommand)]
enum QueryCommand {
    /// Append one annotated query to a query file and generate; the file
    /// keeps it only when the generation succeeds
    #[command(visible_alias = "a")]
    Add {
        /// The query's name, in PascalCase: `OwnedMedia` generates `owned_media`
        #[arg(value_name = "NAME")]
        name: String,

        /// The SQL, with `$1`, `$2`, … placeholders
        #[arg(value_name = "SQL")]
        sql: String,

        /// The query file: a stem (`media`) under the configured `queries`
        /// directory, or a path ending in `.sql`
        #[arg(short, long, value_name = "FILE")]
        file: Option<String>,

        #[command(flatten)]
        mode: ModeArgs,

        /// Read options from this config file instead of discovering
        /// rok-data-sqlc.toml in the current directory or its parents
        #[arg(short, long, value_name = "FILE")]
        config: Option<PathBuf>,
    },
}

/// The clap definition of the `rok-data-sqlc` command line.
pub fn command() -> clap::Command {
    <Cli as clap::CommandFactory>::command()
}

pub fn run() -> Result<()> {
    run_from(std::env::args_os())
}

/// Opens a [`Describer`](crate::check_db::Describer) on the database at a
/// URL, for `check --db`; the `rok` binary hands one to [`run_with`].
pub type Connect = dyn Fn(&str) -> Result<Box<dyn crate::check_db::Describer>>;

pub fn run_from<I, T>(args: I) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    run_inner(args, None)
}

/// [`run_from`], with `connect` opening the database `check --db` checks
/// against.
pub fn run_with<I, T>(args: I, connect: &Connect) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    run_inner(args, Some(connect))
}

fn run_inner<I, T>(args: I, connect: Option<&Connect>) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(err) if is_informational(&err) => {
            // `--help` / `--version` are successful outcomes, not errors.
            err.print()?;
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };
    match cli.command {
        Command::CheckArch { lenient, config } => check_arch(config, lenient),
        Command::Check {
            db,
            lenient,
            config,
        } => check::check(
            &GenerateArgs::checking(config, lenient),
            db.as_deref(),
            connect,
        ),
        Command::Query {
            action:
                QueryCommand::Add {
                    name,
                    sql,
                    file,
                    mode,
                    config,
                },
        } => {
            let add = crate::query_add::QueryAdd {
                name: name.clone(),
                sql,
                file,
                mode: mode.mode(),
                config,
            };
            let (path, report) = crate::query_add::run(add)?;
            print_report(&Config::default(), &report);
            println!("added `{name}` to {}", display_path(&path));
            Ok(())
        }
        Command::Init {
            dir,
            template: Some(template),
            ..
        } => {
            let scaffolded = crate::init::init_template(&dir, &template)?;
            let monorepo = rok_templates::catalog::apps(&template)
                .iter()
                .map(|app| app.name)
                .collect::<Vec<_>>();
            let mut steps = Vec::new();
            if !scaffolded.wrote_manifest {
                let step = if monorepo.is_empty() {
                    "add the template's dependencies and lints (its README.md lists them) to Cargo.toml"
                } else {
                    "add the apps' dependencies and lints (each app's README.md lists them) to its manifest"
                };
                steps.push(step);
            }
            print_scaffolded(scaffolded);
            if monorepo.is_empty() {
                steps.extend([
                    "run `rok sqlc generate` to generate src/db/, src/shared/ and src/features/ from schema/",
                    "set DATABASE_URL and JWT_SECRET (see .env.example) and apply migrations/",
                    "`cargo test`, then `cargo run`",
                ]);
            } else {
                steps.extend([
                    "run `rok gen` from the root to generate every named App from its own schema",
                    "set DATABASE_URL and JWT_SECRET for apps/api (see its .env.example) and apply apps/api/migrations/",
                    "`cargo test` in apps/api, then `cargo run -p axum-api`",
                    "in apps/web, `bun install`, then `bun run dev` (it forwards /api to apps/api)",
                ]);
            }
            print_next_steps(&steps);
            Ok(())
        }
        Command::Init {
            dir,
            template: None,
            api_framework,
        } => {
            let framework = api_framework.map_or(ApiFramework::ActixWeb, ApiFramework::from);
            print_scaffolded(crate::init::init(&dir, framework)?);
            print_next_steps(&[
                "add the dependencies listed at the end of rok-data-sqlc.toml to Cargo.toml",
                "add truth files to schema/ (0001_init.sql, ...)",
                "run `rok sqlc generate` to generate src/db/ and src/features/",
                "set DATABASE_URL and JWT_SECRET, then `cargo run`",
            ]);
            Ok(())
        }
        Command::Generate(args) => generate::run(&args),
    }
}

/// `check-arch`: checks the project whose config file is `config` (or the
/// discovered `rok-data-sqlc.toml`), printing violations on stdout and, with
/// `lenient`, layout violations as warnings on stderr.
fn check_arch(config: Option<PathBuf>, lenient: bool) -> Result<()> {
    let path = match config {
        Some(path) => path,
        None => config_file::discover(&std::env::current_dir()?).with_context(|| {
            format!(
                "no {} found in the current directory or its parents",
                config_file::FILE_NAME
            )
        })?,
    };
    let api = config_file::load(&path)?.api.resolve().with_context(|| {
        format!(
            "{} sets no `api.output`: check-arch checks an API project",
            display_path(&path)
        )
    })?;
    let project = crate::check_arch::Project {
        root: path.parent().unwrap_or(Path::new("")).to_path_buf(),
        features: api.output,
        db_path: api.db_path,
        framework: api.framework,
    };
    let mut failures = 0;
    for violation in crate::check_arch::check(&project)? {
        if lenient && violation.rule.is_layout() {
            eprintln!("warning: {violation}");
        } else {
            failures += 1;
            println!("{violation}");
        }
    }
    if failures > 0 {
        bail!("check-arch found {failures} violation(s)");
    }
    Ok(())
}

/// The usage error for a run with neither `--schema` nor a `schema` key in
/// the JSON payload or the config file.
pub(super) fn missing_schema() -> clap::Error {
    let mut cmd = command();
    cmd.build();
    let generate = cmd
        .find_subcommand_mut("generate")
        .expect("the generate subcommand exists");
    generate.error(
        clap::error::ErrorKind::MissingRequiredArgument,
        format!(
            "no schema given: pass --schema <PATH>, or set `schema` in the --json payload or {}",
            config_file::FILE_NAME
        ),
    )
}

fn is_informational(err: &clap::Error) -> bool {
    use clap::error::ErrorKind;
    matches!(
        err.kind(),
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
    )
}
