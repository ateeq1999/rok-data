//! `rok-data` — the command-line interface for the rok-data ecosystem,
//! run on its own or linked into rok-v2's `rok` (workspace ADR 0032), which
//! is how users reach it and why the commands below read `rok …`.
//!
//! Every user-facing command of the workspace lives here; the
//! crates it drives expose the same operations as reusable library APIs
//! (`rok-data`, `rok-data-codegen`, `rok-data-sqlc`), so scripts and tests can
//! call them without a shell.
//!
//! ```text
//! rok init [NAME]                # scaffold a project (skeleton or template)
//! rok gen [OPTIONS] [check]      # data layer and API features from the schema dir
//! rok generate                  # legacy: model crate from .sql files
//! rok generate check            # CI: are the generated files up to date?
//! rok migrate create|status|up|down
//! rok db check|inspect
//! rok schema dump|diff
//! rok sqlc generate|init|check-arch   # legacy: forwarded to the engine
//! ```
//!
//! `rok tools …` is rok-v2's: its `rok` links rok-tools beside this CLI
//! (workspace ADR 0032).
//!
//! `rok generate` and `rok sqlc …` are the legacy surfaces
//! (workspace ADR 0012): their flags, their output and their exit
//! codes are unchanged — `rok sqlc …` forwards its tokens verbatim to
//! the engine, and `rok generate` no longer takes the `generate`
//! subcommand — and the root help marks them as superseded by the
//! front door, their removal deferred to 1.0.

use std::process::ExitCode;

mod cli;
pub mod commands;
pub mod config;
mod context;
mod group;
mod keychain;
mod output;

pub use group::Group;

/// Run the CLI with `args` (including the program name, like
/// `std::env::args().collect()`), returning the process exit code.
///
/// The program name is what help and usage errors display: the `rok-data`
/// binary passes its own path, and rok-v2's `rok` passes `"rok"` (workspace
/// ADR 0032).
///
/// Public so the integration tests can drive the CLI as a library call, so
/// `main` delegates to exactly this behavior, and so `rok` links it.
pub fn run<I, T>(args: I) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<String> + Clone,
{
    run_in(None, args.into_iter().map(Into::into).collect())
}

/// [`run`], answering only `group`'s commands: what the `rok-db` and
/// `rok-gen` binaries run. A command of the other group is a usage error
/// (exit 2) naming the binary that answers it.
pub fn run_group<I, T>(group: Group, args: I) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<String> + Clone,
{
    run_in(Some(group), args.into_iter().map(Into::into).collect())
}

fn run_in(group: Option<Group>, args: Vec<String>) -> ExitCode {
    let fallback = group.map_or("rok-data", Group::binary);
    let program = program_name(&args, fallback);
    if let (Some(group), Some(word)) = (group, args.get(1))
        && let Some(refusal) = cli::canonical(word).and_then(|name| group.refusal(&program, &name))
    {
        eprintln!("{refusal}");
        return ExitCode::from(2);
    }
    match route_sqlc(&program, &args) {
        Outcome::Handled(code) => code,
        Outcome::Continue => run_root(&program, group, args),
    }
}

/// Every command, with its help, as `rok-data` displays it — what a caller
/// linking this CLI (rok-v2's `rok`) embeds in its own help.
#[must_use]
pub fn command() -> clap::Command {
    cli::command("rok-data", None)
}

/// `group`'s commands, with their help, as its binary displays them.
#[must_use]
pub fn group_command(group: Group) -> clap::Command {
    cli::command(group.binary(), Some(group))
}

/// The name to display: the file stem of `args[0]` (`rok-data` from
/// `C:\…\rok-data.exe`), or `fallback` when there is none.
fn program_name(args: &[String], fallback: &str) -> String {
    args.first()
        .and_then(|arg| std::path::Path::new(arg).file_stem())
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or(fallback)
        .to_owned()
}

/// What `route_sqlc` (the sqlc-token router below) decided.
#[derive(Debug)]
pub enum Outcome {
    /// The `sqlc` command handled everything (including clap errors), exit here.
    Handled(ExitCode),
    /// Not an `sqlc` invocation; keep going with the main dispatch.
    Continue,
}

use Outcome::{Continue, Handled};

/// `rok sqlc …` reuses the config resolution and clap grammar that the
/// `rok-data-sqlc` engine tests also exercise, so their definitions live in
/// that crate. Everything else is parsed here.
fn route_sqlc(program: &str, args: &[String]) -> Outcome {
    if args.len() >= 2 && args[1] == "sqlc" {
        return Handled(commands::sqlc::run(program, &args[2..]));
    }
    Continue
}

fn run_root(program: &str, group: Option<Group>, args: Vec<String>) -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("error: can't start the async runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    match cli::parse(program, group, &args[1..]) {
        cli::ParseOutcome::Informational => ExitCode::SUCCESS,
        cli::ParseOutcome::Usage => ExitCode::from(2),
        cli::ParseOutcome::Command(command) => {
            let result = runtime.block_on(commands::execute(command));
            runtime.shutdown_timeout(std::time::Duration::from_millis(100));
            match result {
                Ok(code) => code,
                Err(message) => {
                    eprintln!("error: {message}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}
