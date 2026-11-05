//! Where the commands that work in one directory (`rok migrate *`, `rok
//! schema diff`) find their migration files: `--dir`, else the project the
//! one rule finds (`rok_data_sqlc::Project`). Each says what it read in one
//! line on stderr, so `--json` stdout stays clean.

use std::path::PathBuf;

use rok_data_sqlc::{Project, using_line};

use crate::cli::migrate::Place;

/// The migration history directory `place` names, after printing the line
/// that says where it came from.
pub(crate) fn migrations_dir(place: &Place) -> Result<PathBuf, String> {
    let (line, dir) = match &place.dir {
        Some(dir) => (using_line("--dir", &[("migrations", dir)]), dir.clone()),
        None => {
            let target = Project::current()
                .and_then(|project| project.target(place.app.as_deref()))
                .map_err(|error| format!("{error:#}"))?;
            let dir = target
                .migrations_dir()
                .map_err(|error| format!("{error:#}"))?;
            (target.using(&[("migrations", &dir)]), dir)
        }
    };
    eprintln!("{line}");
    Ok(dir)
}
