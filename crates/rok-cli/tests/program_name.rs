//! Help and usage show the name the CLI was called by: `rok-data` on its
//! own, `rok` when rok-v2 links it (workspace ADR 0032).

use std::process::Command;

use anyhow::Result;

fn help_of(bin: &std::path::Path, args: &[&str]) -> Result<String> {
    let output = Command::new(bin).args(args).output()?;
    assert!(output.status.success(), "{output:?}");
    Ok(String::from_utf8(output.stdout)?)
}

#[test]
fn the_binary_calls_itself_rok_data() -> Result<()> {
    let bin = std::path::Path::new(env!("CARGO_BIN_EXE_rok-data"));
    assert!(help_of(bin, &["--help"])?.contains("Usage: rok-data <COMMAND>"));
    assert!(help_of(bin, &["sqlc", "--help"])?.contains("Usage: rok-data sqlc"));
    Ok(())
}

#[test]
fn a_copy_named_rok_calls_itself_rok() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let rok = dir
        .path()
        .join(format!("rok{}", std::env::consts::EXE_SUFFIX));
    std::fs::copy(env!("CARGO_BIN_EXE_rok-data"), &rok)?;
    let help = help_of(&rok, &["--help"])?;
    assert!(help.contains("Usage: rok <COMMAND>"), "{help}");
    assert!(!help.contains("rok-data <COMMAND>"), "{help}");
    assert!(help_of(&rok, &["sqlc", "--help"])?.contains("Usage: rok sqlc"));
    Ok(())
}

#[test]
fn command_lists_every_root_command() {
    let command = rok_data_cli::command();
    assert_eq!(command.get_name(), "rok-data");
    let names: Vec<&str> = command.get_subcommands().map(|c| c.get_name()).collect();
    for expected in [
        "init", "gen", "generate", "migrate", "db", "schema", "server", "sqlc",
    ] {
        assert!(
            names.contains(&expected),
            "{expected} missing from {names:?}"
        );
    }
}
