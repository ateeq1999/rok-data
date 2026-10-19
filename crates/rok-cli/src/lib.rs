//! `rok` — the single command-line interface for the rok-data ecosystem.
//!
//! Every user-facing command of the consolidated workspace lives here; the
//! crates it drives expose the same operations as reusable library APIs
//! (`rok-data`, `rok-data-codegen`, `rok-data-sqlc`), so scripts and tests can
//! call them without a shell.
//!
//! ```text
//! rok generate                  # model crate from .sql files
//! rok generate check            # CI: are the generated files up to date?
//! rok migrate create|status|up|down
//! rok db check|inspect
//! rok schema dump|diff
//! rok sqlc generate|init|check-arch
//! ```

use std::process::ExitCode;

mod cli;
pub mod commands;
mod config;
mod context;
mod output;

/// Run the CLI with `args` (including the program name, like
/// `std::env::args().collect()`), returning the process exit code.
///
/// Public so the integration tests can drive the CLI as a library call and so
/// `main` delegates to exactly this behavior.
pub fn run<I, T>(args: I) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<String> + Clone,
{
    let args: Vec<String> = args.into_iter().map(Into::into).collect();
    match route_sqlc(&args) {
        Outcome::Handled(code) => code,
        Outcome::Continue => run_root(args),
    }
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
fn route_sqlc(args: &[String]) -> Outcome {
    if args.len() >= 2 && args[1] == "sqlc" {
        return Handled(commands::sqlc::run(&args[2..]));
    }
    Continue
}

fn run_root(args: Vec<String>) -> ExitCode {
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
    match cli::parse(&args[1..]) {
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
