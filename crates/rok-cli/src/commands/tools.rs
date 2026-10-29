//! `rok tools …` — the rok-tools developer tools (ADR 0026).
//!
//! Like `rok sqlc …`, the tokens are forwarded verbatim to
//! `rok_tools_cli::run_from`, so `rok tools <tool>` and the standalone
//! `rok-tools <tool>` share one clap grammar, help text and exit codes
//! (0 ok, 1 a Tool failed or a check found violations, 2 usage/config error).

use std::process::ExitCode;

/// Run `rok tools` with `args` (not including the `rok tools` tokens).
pub(crate) fn run(args: &[String]) -> ExitCode {
    // The program token doubles as the displayed bin name in help and errors.
    let mut full: Vec<String> = vec!["rok tools".into()];
    full.extend(args.iter().cloned());
    let code = rok_tools_cli::run_from(full);
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forwards_to_rok_tools() {
        assert_eq!(run(&["list".into()]), ExitCode::SUCCESS);
        assert_eq!(run(&["no-such-tool".into()]), ExitCode::from(2));
    }
}
