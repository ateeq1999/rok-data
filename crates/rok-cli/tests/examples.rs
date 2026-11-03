//! The examples `--help` shows (`rok_data_cli::examples`) are real: each
//! one parses against the grammar, each names a command that exists, and
//! every command has some. The `sqlc` subtree is the engine's frozen
//! grammar (invariant 3) and has its own help.

use rok_data_cli::examples::{EXAMPLES, examples};

/// `line` split the way a shell splits it here: on spaces, with
/// `'single quoted'` runs kept whole.
fn words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut started = false;
    for c in line.chars() {
        match c {
            '\'' => {
                quoted = !quoted;
                started = true;
            }
            ' ' if !quoted => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            _ => {
                word.push(c);
                started = true;
            }
        }
    }
    assert!(!quoted, "unclosed quote in `{line}`");
    if started {
        words.push(word);
    }
    words
}

/// Every command path below the root, but `sqlc`'s.
fn command_paths() -> Vec<String> {
    fn walk(command: &clap::Command, prefix: &str, out: &mut Vec<String>) {
        for sub in command.get_subcommands() {
            let path = if prefix.is_empty() {
                sub.get_name().to_owned()
            } else {
                format!("{prefix} {}", sub.get_name())
            };
            if path == "sqlc" {
                continue;
            }
            out.push(path.clone());
            walk(sub, &path, out);
        }
    }
    let mut out = Vec::new();
    walk(&rok_data_cli::command(), "", &mut out);
    out
}

/// A group (`db`, `migrate`) holds commands; it is not run on its own.
fn is_group(path: &str) -> bool {
    let command = rok_data_cli::command();
    let found = path.split(' ').try_fold(command, |command, name| {
        command.find_subcommand(name).cloned()
    });
    found.is_some_and(|command| command.is_subcommand_required_set())
}

#[test]
fn every_example_parses_against_the_grammar() {
    for (path, list) in EXAMPLES {
        for example in *list {
            let mut args = words(example.command);
            assert_eq!(
                args.first().map(String::as_str),
                Some("rok"),
                "`{}` does not start with `rok`",
                example.command
            );
            args[0] = "rok-data".to_owned();
            let parsed = rok_data_cli::command().try_get_matches_from(&args);
            assert!(
                parsed.is_ok(),
                "{path}: `{}` does not parse: {}",
                example.command,
                parsed.err().map(|err| err.to_string()).unwrap_or_default()
            );
            assert!(
                example.command.starts_with(&format!("rok {path}")),
                "{path}: `{}` runs another command",
                example.command
            );
        }
    }
}

#[test]
fn every_runnable_command_has_examples() {
    for path in command_paths() {
        if is_group(&path) {
            continue;
        }
        assert!(
            examples(&path).is_some_and(|list| !list.is_empty()),
            "`rok {path}` has no examples in rok_data_cli::examples::EXAMPLES"
        );
    }
}

#[test]
fn every_example_names_a_command_that_exists() {
    let paths = command_paths();
    for (path, _) in EXAMPLES {
        assert!(
            paths.iter().any(|known| known == path),
            "`{path}` is not a command"
        );
    }
}

#[test]
fn examples_show_in_the_long_help_only() {
    let mut command = rok_data_cli::command();
    let create = command
        .find_subcommand_mut("db")
        .and_then(|db| db.find_subcommand_mut("create"))
        .expect("rok db create");
    let long = create.render_long_help().to_string();
    let short = create.render_help().to_string();
    assert!(long.contains("Examples:\n  # Create a database"), "{long}");
    assert!(!short.contains("Examples:"), "{short}");
}
