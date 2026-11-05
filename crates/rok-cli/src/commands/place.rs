//! Where the commands that work in one directory (`rok migrate *`, `rok
//! schema diff`) find their migration files: `--dir`, else the project the
//! one rule finds (`rok_data_sqlc::Project`). Each says what it read in one
//! line on stderr, so `--json` stdout stays clean.

use std::path::PathBuf;

use rok_data_sqlc::{Project, using_line};

use crate::cli::EnvFile;
use crate::cli::migrate::Place;
use crate::commands::env_file;

/// The migration history directory `place` names, after printing the line
/// that says where it came from. With `env` (the commands that connect),
/// the App's `.env` is loaded first and the line ends `, loaded
/// apps/api/.env`; under `--dir` that is the `.env` of the project around
/// the current directory, if any.
pub(crate) fn migrations_dir(place: &Place, env: Option<EnvFile>) -> Result<PathBuf, String> {
    let (line, dir, target) = match &place.dir {
        Some(dir) => (
            using_line("--dir", &[("migrations", dir)]),
            dir.clone(),
            env.and_then(|_| env_file::current_target()),
        ),
        None => {
            let target = Project::current()
                .and_then(|project| project.target(place.app.as_deref()))
                .map_err(|error| format!("{error:#}"))?;
            let dir = target
                .migrations_dir()
                .map_err(|error| format!("{error:#}"))?;
            (target.using(&[("migrations", &dir)]), dir, Some(target))
        }
    };
    let loaded = match env {
        Some(env) => env_file::load(env, target.as_ref())?,
        None => None,
    };
    match loaded {
        Some(loaded) => eprintln!("{line}, loaded {loaded}"),
        None => eprintln!("{line}"),
    }
    Ok(dir)
}
