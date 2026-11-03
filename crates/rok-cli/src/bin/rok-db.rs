//! `rok-db`: rok-data's database commands (`server`, `db`, `migrate`,
//! `schema`) on their own; the library's `run_group` does everything.

fn main() -> std::process::ExitCode {
    rok_data_cli::run_group(rok_data_cli::Group::Db, std::env::args())
}
