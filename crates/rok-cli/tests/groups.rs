//! `rok-db` and `rok-gen`: the CLI restricted to one group of commands
//! each, listing only those and refusing the other group's with the
//! binary that answers them (workspace ADR 0032).

use std::process::{Command, Output};

fn run(bin: &str, args: &[&str]) -> Output {
    Command::new(bin)
        .args(args)
        .output()
        .expect("run the binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The command names `--help` lists.
fn listed(help: &str) -> Vec<String> {
    help.lines()
        .skip_while(|line| *line != "Commands:")
        .skip(1)
        .take_while(|line| line.starts_with("  "))
        .filter_map(|line| line.split_whitespace().next().map(str::to_owned))
        .collect()
}

#[test]
fn rok_db_lists_only_the_database_commands() {
    let output = run(env!("CARGO_BIN_EXE_rok-db"), &["--help"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let help = stdout(&output);
    assert!(help.contains("Usage: rok-db <COMMAND>"), "{help}");
    let mut commands = listed(&help);
    commands.sort();
    assert_eq!(commands, ["db", "migrate", "schema", "server"], "{help}");
}

#[test]
fn rok_gen_lists_only_the_generators() {
    let output = run(env!("CARGO_BIN_EXE_rok-gen"), &["--help"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let help = stdout(&output);
    assert!(help.contains("Usage: rok-gen <COMMAND>"), "{help}");
    let mut commands = listed(&help);
    commands.sort();
    assert_eq!(commands, ["gen", "generate", "init", "sqlc"], "{help}");
}

#[test]
fn each_refuses_the_others_commands_naming_the_binary_that_answers() {
    for (bin, word, answers) in [
        (env!("CARGO_BIN_EXE_rok-db"), "gen", "rok-gen gen"),
        (env!("CARGO_BIN_EXE_rok-db"), "sqlc", "rok-gen sqlc"),
        (env!("CARGO_BIN_EXE_rok-gen"), "migrate", "rok-db migrate"),
        (env!("CARGO_BIN_EXE_rok-gen"), "server", "rok-db server"),
    ] {
        let output = run(bin, &[word, "--help"]);
        assert_eq!(output.status.code(), Some(2), "{bin} {word}");
        let message = stderr(&output);
        assert!(
            message.contains(&format!("`{answers}`")) && message.contains(&format!("`rok {word}`")),
            "{bin} {word}: {message}"
        );
        assert!(stdout(&output).is_empty(), "{bin} {word} printed help");
    }
}

#[test]
fn each_answers_its_own_commands_under_its_name() {
    for (bin, args, usage) in [
        (
            env!("CARGO_BIN_EXE_rok-db"),
            &["migrate", "--help"][..],
            "Usage: rok-db migrate",
        ),
        (
            env!("CARGO_BIN_EXE_rok-db"),
            &["db", "--help"][..],
            "Usage: rok-db db",
        ),
        (
            env!("CARGO_BIN_EXE_rok-gen"),
            &["gen", "--help"][..],
            "Usage: rok-gen gen",
        ),
        (
            env!("CARGO_BIN_EXE_rok-gen"),
            &["sqlc", "--help"][..],
            "Usage: rok-gen sqlc",
        ),
    ] {
        let output = run(bin, args);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{args:?}: {}",
            stderr(&output)
        );
        assert!(
            stdout(&output).contains(usage),
            "{args:?}:\n{}",
            stdout(&output)
        );
    }
}

/// An unknown word is clap's usage error (exit 2), not a refusal.
#[test]
fn an_unknown_word_is_a_usage_error() {
    let output = run(env!("CARGO_BIN_EXE_rok-db"), &["nope"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        !stderr(&output).contains("has no command"),
        "{}",
        stderr(&output)
    );
}

/// The groups cover every command `rok-data` has, each exactly once.
#[test]
fn the_groups_split_every_command() {
    let mut grouped: Vec<&str> = [rok_data_cli::Group::Db, rok_data_cli::Group::Gen]
        .iter()
        .flat_map(|group| group.commands().iter().copied())
        .collect();
    grouped.sort_unstable();
    let mut all: Vec<String> = rok_data_cli::command()
        .get_subcommands()
        .map(|command| command.get_name().to_owned())
        .collect();
    all.sort();
    assert_eq!(grouped, all);
}

/// `group_command` is what each binary's help is built from.
#[test]
fn group_command_names_the_binary() {
    assert_eq!(
        rok_data_cli::group_command(rok_data_cli::Group::Db).get_display_name(),
        Some("rok-db")
    );
    assert_eq!(rok_data_cli::Group::Gen.binary(), "rok-gen");
}
