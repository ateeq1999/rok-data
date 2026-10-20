//! `rok sqlc generate|init|check-arch` — the SQLx module generator from
//! `rok-data-sqlc`.
//!
//! The engine carries its own user-tested clap grammar (`run_from`, used by
//! its tests) and this module is the single user-facing entry point: same
//! arguments, same output, same exit codes after the consolidation. Keeping
//! the grammar inside the engine means its test corpus verifies the exact
//! surface the CLI ships; the interface here owns dispatch and exit codes.
//!
//! The subtree is the legacy surface (ADR 0012): superseded by the front
//! door (`rok init`, `rok gen`) and marked as such in the root help, but
//! still forwarded to the engine verbatim — the tokens, the help text and
//! the exit codes are the engine's own. Its removal is deferred to 1.0.

use std::process::ExitCode;

/// Run the `sqlc` subcommand with `args` (not including the `rok sqlc`
/// program tokens). Mirrors the pre-consolidation `rok-sqlc` main binary:
/// `--help`/`--version` succeed, clap usage errors exit 1 with clap's own
/// message, engine errors exit 1.
pub(crate) fn run(args: &[String]) -> ExitCode {
    // The engine's derive parses [program, subcommand…]; the program token
    // doubles as the displayed bin name, so it says `rok sqlc` the way the
    // consolidated CLI presents this group.
    let mut full: Vec<String> = vec!["rok sqlc".into()];
    full.extend(args.iter().cloned());
    match rok_data_sqlc::cli::run_from(full) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            // Usage errors carry clap's own formatted message, prefix included.
            match err.downcast_ref::<clap::Error>() {
                Some(usage) => {
                    let _ = usage.print();
                }
                None => eprintln!("Error: {err:?}"),
            }
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_exits_successfully() {
        let code = run(&["--help".to_owned()]);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn usage_errors_exit_one() {
        let code = run(&["nope".to_owned()]);
        assert_eq!(code, ExitCode::FAILURE);
    }
}
