//! `rok gen`'s "Next steps" print commands that run as printed, from the
//! directory `rok gen` ran in: the Monorepo's root or inside its App.

use super::*;

/// The commands a "Next steps" block prints: each step's backticked
/// `rok ...` text.
fn printed_commands(stdout: &str) -> Vec<String> {
    let (_, steps) = stdout.split_once("Next steps:").unwrap_or_default();
    steps
        .split('`')
        .skip(1)
        .step_by(2)
        .filter(|text| text.starts_with("rok "))
        .map(str::to_owned)
        .collect()
}

/// Runs a printed `rok ...` command in `dir`, against a database that
/// isn't there: `rok migrate up` says which directory it read, then fails
/// to connect.
fn run_printed(dir: &Path, command: &str) -> Result<Output> {
    let args: Vec<&str> = command.split_whitespace().skip(1).collect();
    Ok(rok()
        .current_dir(dir)
        .args(&args)
        .env("DATABASE_URL", "postgres://rok:rok@127.0.0.1:1/rok")
        .output()?)
}

/// A Monorepo whose App `api` keeps its truth in `schema/`.
fn monorepo() -> Result<TempDir> {
    let dir = TempDir::new()?;
    fs::write(dir.path().join("rok.toml"), "[apps]\napi = \"apps/api\"\n")?;
    let app = dir.path().join("apps/api");
    fs::create_dir_all(app.join("schema"))?;
    fs::write(app.join("rok-data-sqlc.toml"), "schema = \"schema\"\n")?;
    fs::write(app.join("schema/0001_init.sql"), SCHEMA)?;
    Ok(dir)
}

/// Each printed command works from where `rok gen` ran: `rok migrate up`
/// (no `--dir`: it finds the App's migrations itself) reads the pair just
/// staged, and `rok gen check` passes.
fn assert_runs_as_printed(dir: &Path, migrations: &str) -> Result<()> {
    let output = run_in(dir, &["gen", "--migration", "init"])?;
    assert_success(&output);
    let commands = printed_commands(&stdout(&output));
    assert_eq!(
        commands,
        vec!["rok migrate up".to_owned(), "rok gen check".to_owned()],
        "{}",
        stdout(&output)
    );
    let up = run_printed(dir, &commands[0])?;
    let line = format!("(migrations {migrations})");
    assert!(stderr(&up).contains(&line), "{}", stderr(&up));
    assert_success(&run_printed(dir, &commands[1])?);
    Ok(())
}

#[test]
fn next_steps_run_as_printed_from_the_root() -> Result<()> {
    let dir = monorepo()?;
    assert_runs_as_printed(dir.path(), "apps/api/migrations")
}

#[test]
fn next_steps_run_as_printed_inside_the_app() -> Result<()> {
    let dir = monorepo()?;
    assert_runs_as_printed(&dir.path().join("apps/api"), "migrations")
}

/// A history dir only the flag names is named again: `rok migrate up`
/// can't know it otherwise.
#[test]
fn a_history_dir_only_the_flag_names_is_printed() -> Result<()> {
    let dir = project(SCHEMA)?;
    let output = run_in(
        dir.path(),
        &["gen", "--migrations-dir", "history", "--migration", "init"],
    )?;
    assert_success(&output);
    let commands = printed_commands(&stdout(&output));
    assert_eq!(
        commands[0],
        "rok migrate up --dir history",
        "{}",
        stdout(&output)
    );
    let up = run_printed(dir.path(), &commands[0])?;
    assert!(
        stderr(&up).contains("(migrations history)"),
        "{}",
        stderr(&up)
    );
    Ok(())
}

/// `rok init my-app` prints the `cd` into the project first, so the
/// `rok` commands after it run as printed.
#[test]
fn init_next_steps_run_as_printed() -> Result<()> {
    let parent = TempDir::new()?;
    let output = run_in(parent.path(), &["init", "my-app"])?;
    assert_success(&output);
    let stdout = stdout(&output);
    let (_, steps) = stdout.split_once("Next steps:").unwrap_or_default();
    let commands: Vec<&str> = steps
        .split('`')
        .skip(1)
        .step_by(2)
        .filter(|text| text.starts_with("rok ") || text.starts_with("cd "))
        .collect();
    assert_eq!(
        commands,
        vec!["cd my-app", "rok gen --migration init", "rok gen"],
        "{stdout}"
    );
    // The step between: describe the tables in schema/.
    let mut dir = parent.path().to_path_buf();
    for command in commands {
        if let Some(target) = command.strip_prefix("cd ") {
            dir = dir.join(target);
            fs::write(dir.join("schema/0001_init.sql"), SCHEMA)?;
            continue;
        }
        assert_success(&run_printed(&dir, command)?);
    }
    Ok(())
}

/// Scaffolded in the current directory, there is nothing to `cd` into.
#[test]
fn init_here_prints_no_cd() -> Result<()> {
    let dir = TempDir::new()?;
    let output = run_in(dir.path(), &["init"])?;
    assert_success(&output);
    assert!(!stdout(&output).contains("`cd "), "{}", stdout(&output));
    Ok(())
}
