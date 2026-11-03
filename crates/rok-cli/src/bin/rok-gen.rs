//! `rok-gen`: rok-data's generators (`init`, `gen`, `generate`, `sqlc`) on
//! their own; the library's `run_group` does everything.

fn main() -> std::process::ExitCode {
    rok_data_cli::run_group(rok_data_cli::Group::Gen, std::env::args())
}
