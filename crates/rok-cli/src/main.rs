//! `rok` binary entry point: delegate to the library `run`.

fn main() -> std::process::ExitCode {
    rok_cli::run(std::env::args())
}
