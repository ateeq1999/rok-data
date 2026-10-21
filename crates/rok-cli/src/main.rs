//! `rok` binary entry point: delegate to the library `run`.

fn main() -> std::process::ExitCode {
    rok_data_cli::run(std::env::args())
}
