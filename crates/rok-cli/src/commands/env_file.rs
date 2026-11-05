//! The App's `.env`: the database commands (`rok migrate *`, `rok db *`,
//! `rok schema *`) load the `.env` of the App the project rule finds (the
//! one beside a plain project's settings file), so `DATABASE_URL` set there
//! needs no `export`. A variable already in the environment wins over the
//! file's; `--no-env-file` reads nothing.

use rok_data_sqlc::{Project, Target};

use crate::cli::EnvFile;
use crate::commands::chain;

/// The place the current directory's project acts on, when there is one
/// and it is one App or a plain project; never an error, for commands that
/// run without a project.
pub(crate) fn current_target() -> Option<Target> {
    Project::current().ok()?.target(None).ok()
}

/// Loads `target`'s `.env` unless `env` opts out, and returns the file it
/// loaded, as the `using` line shows it (`apps/api/.env`). Nothing is
/// loaded without a project (a stray `.env` is not an App's), or when the
/// file is not there.
pub(crate) fn load(env: EnvFile, target: Option<&Target>) -> Result<Option<String>, String> {
    if env.no_env_file {
        return Ok(None);
    }
    let Some(target) = target.filter(|target| target.app.is_some() || target.settings.is_some())
    else {
        return Ok(None);
    };
    let path = target.dir.join(".env");
    if !path.is_file() {
        return Ok(None);
    }
    let shown = path.display().to_string().replace('\\', "/");
    // `from_path` sets only the variables not already set: the
    // environment wins.
    dotenvy::from_path(&path).map_err(|error| format!("{shown}: {}", chain(error)))?;
    Ok(Some(shown))
}

/// [`load`] for a command with no `using` line of its own (`rok db *`, `rok
/// schema dump`): the file it loaded, said on stderr.
pub(crate) fn load_for_current(env: EnvFile) -> Result<(), String> {
    if env.no_env_file {
        return Ok(());
    }
    if let Some(loaded) = load(env, current_target().as_ref())? {
        eprintln!("loaded {loaded}");
    }
    Ok(())
}
