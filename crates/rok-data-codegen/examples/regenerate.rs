//! Regenerate the committed example with the codegen engine directly.
//!
//! Contributors run this from the repository root:
//!
//! ```text
//! cargo run -p rok-data-codegen --example regenerate
//! ```
//!
//! The user-facing path is `rok generate --config examples/sqlgen/rok-data.toml`
//! (which reads the settings file); this example hard-codes the same settings
//! so the committed example can be refreshed without the CLI installed.

use std::path::PathBuf;

use rok_data_codegen::{Dependency, Options, differences, generate, write};

fn main() {
    let mut options = Options::new();
    options.input = PathBuf::from("examples/sqlgen/db");
    options.output = PathBuf::from("examples/sqlgen/db-gen");
    options.crate_name = "sqlgen-db".to_owned();
    // Same settings as `examples/sqlgen/rok-data.toml`.
    options.rok_data = Dependency::Path(PathBuf::from(
        "examples/sqlgen/../../crates/rok-data",
    ));
    options.strict = false;
    options.database_url = std::env::var("DATABASE_URL").ok().filter(|u| !u.is_empty());

    let generated = generate(&options).expect("generate");
    for warning in &generated.warnings {
        eprintln!("warning: {warning}");
    }
    if let Some(name) = &generated.new_migration {
        eprintln!("added migration `{name}` (commit it, or clean the directory and retry)");
    }
    let written = write(&options, &generated).expect("write");
    if written.is_empty() {
        println!("{} is up to date", options.output.display());
    } else {
        for path in written {
            println!("wrote {}", options.output.join(path).display());
        }
    }
    let stale = differences(&options, &generated);
    assert!(stale.is_empty(), "still out of date: {stale:?}");
}
