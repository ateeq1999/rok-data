//! The project a command acts on, and its settings file.
//!
//! Two settings shapes are readable, and both name the same keys:
//!
//! - the unified project file `rok.toml` (engine ADR 0011), whose generation
//!   settings live in its `[generate]` section, beside the `[api]` and
//!   `[conventions]` sections;
//! - the engine's own file `rok-data-sqlc.toml`, whose keys sit at the top
//!   level.
//!
//! Paths in the file resolve against the file's directory; the flags beat
//! the file everywhere.
//!
//! # One rule for finding the project
//!
//! Every command that reads a project ([`Project::find`]) walks up from the
//! current directory. A `rok.toml` whose `[apps]` names the Apps of a
//! Monorepo (workspace ADR 0027) makes it a Monorepo: started inside one of
//! those Apps, the command acts on that App; anywhere else under the root,
//! on the App(s) `[apps]` names. Without one, the nearest settings file is a
//! plain project. Each command says what it read in one line on stderr
//! ([`Target::using`]).

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::Config;
use crate::config_file::{self, FileConfig};
use crate::report::display_path;

/// The config file names, in the order they are looked for: the canonical
/// `rok.toml` (engine ADR 0011) and the engine's legacy `rok-data-sqlc.toml`.
pub const FILE_NAMES: &[&str] = &["rok.toml", "rok-data-sqlc.toml"];

/// The project file that can name a Monorepo's Apps.
const PROJECT_FILE: &str = "rok.toml";

/// The history directory a project has when its settings name none.
const DEFAULT_MIGRATIONS_DIR: &str = "migrations";

/// The nearest config file in `start` or one of its parents.
///
/// The canonical name wins over the legacy one in the same directory, and a
/// nearer directory wins over a farther one.
pub fn discover_config(start: &Path) -> Option<PathBuf> {
    start.ancestors().find_map(config_in)
}

/// The settings file in `dir` itself, not its parents.
fn config_in(dir: &Path) -> Option<PathBuf> {
    FILE_NAMES
        .iter()
        .map(|name| dir.join(name))
        .find(|candidate| candidate.is_file())
}

/// The settings the config file at `path` holds.
///
/// The flags override them: the file says what the project wants, the
/// command line says what this run does.
pub fn load_config(path: &Path) -> Result<Config> {
    Ok(config_file::load_any(path)?.into_config())
}

/// The project a command acts on.
#[derive(Debug, Clone)]
pub enum Project {
    /// A project whose settings name no Apps: its settings file, if any.
    Single {
        /// The settings file, or `None` when nothing was found.
        file: Option<PathBuf>,
    },
    /// A Monorepo: a `rok.toml` whose `[apps]` names its Apps.
    Monorepo {
        /// The root `rok.toml`.
        file: PathBuf,
        /// Every App `[apps]` names, in the file's order.
        apps: Vec<App>,
        /// The App the command started inside, an index into `apps`.
        current: Option<usize>,
    },
}

/// An App a Monorepo's `[apps]` names.
#[derive(Debug, Clone)]
pub struct App {
    /// The App's name (`api`).
    pub name: String,
    /// The App's directory.
    pub dir: PathBuf,
    /// The App's own settings file, in its directory, if it has one.
    pub settings: Option<PathBuf>,
}

/// One place a command acts on: a plain project or one App.
#[derive(Debug, Clone)]
pub struct Target {
    /// The App's name, for an App of a Monorepo.
    pub app: Option<String>,
    /// The directory the defaults resolve against (empty: the current one).
    pub dir: PathBuf,
    /// The settings file, if there is one.
    pub settings: Option<PathBuf>,
}

impl Project {
    /// The project the current directory belongs to.
    pub fn current() -> Result<Self> {
        let start = std::env::current_dir().context("can't read the current directory")?;
        Self::find(&start)
    }

    /// The project `start` belongs to: the nearest `rok.toml` naming
    /// `[apps]` above it makes a Monorepo, unless a nearer settings file
    /// outside every App is a plain project of its own; otherwise the
    /// nearest settings file is a plain project.
    pub fn find(start: &Path) -> Result<Self> {
        let nearest = discover_config(start);
        for dir in start.ancestors() {
            let file = dir.join(PROJECT_FILE);
            if !file.is_file() {
                continue;
            }
            let Some(apps) = apps_of(&file)? else {
                continue;
            };
            let current = apps.iter().position(|app| start.starts_with(&app.dir));
            if current.is_none() && nearest.as_deref() != Some(file.as_path()) {
                break;
            }
            return Ok(Self::Monorepo {
                file: relative(&file, start),
                apps: apps
                    .into_iter()
                    .map(|app| App {
                        dir: relative(&app.dir, start),
                        settings: app.settings.map(|path| relative(&path, start)),
                        name: app.name,
                    })
                    .collect(),
                current,
            });
        }
        Ok(Self::Single {
            file: nearest.map(|path| relative(&path, start)),
        })
    }

    /// The project of an explicit settings file (`--config`): a Monorepo
    /// at its root when it names `[apps]`, else a plain project.
    pub fn at(file: &Path) -> Result<Self> {
        Ok(match apps_of(file)? {
            Some(apps) => Self::Monorepo {
                file: file.to_path_buf(),
                apps,
                current: None,
            },
            None => Self::Single {
                file: Some(file.to_path_buf()),
            },
        })
    }

    /// The settings file the project starts from: the plain project's, the
    /// one of the App started inside, or the Monorepo's root `rok.toml`.
    pub fn file(&self) -> Option<&Path> {
        match self {
            Self::Single { file } => file.as_deref(),
            Self::Monorepo {
                apps,
                current: Some(index),
                ..
            } => apps[*index].settings.as_deref(),
            Self::Monorepo { file, .. } => Some(file),
        }
    }

    /// Whether the command started inside one of the Monorepo's Apps.
    pub fn inside_app(&self) -> bool {
        matches!(
            self,
            Self::Monorepo {
                current: Some(_),
                ..
            }
        )
    }

    /// Every place a command that covers the whole project acts on (`rok
    /// gen`, `check-arch`): `--app`'s App, the App started inside, every
    /// App at a Monorepo's root, or the plain project.
    pub fn targets(&self, app: Option<&str>) -> Result<Vec<Target>> {
        match self {
            Self::Single { file } => {
                if let Some(name) = app {
                    bail!(
                        "--app `{name}`: {} names no `[apps]`, so there is no App to pick",
                        file.as_deref()
                            .map_or_else(|| "this project".to_owned(), display_path)
                    );
                }
                Ok(vec![Target {
                    app: None,
                    dir: file
                        .as_deref()
                        .and_then(Path::parent)
                        .map(Path::to_path_buf)
                        .unwrap_or_default(),
                    settings: file.clone(),
                }])
            }
            Self::Monorepo {
                file,
                apps,
                current,
            } => {
                let chosen: Vec<&App> = match (app, current) {
                    (Some(name), _) => vec![named(file, apps, name)?],
                    (None, Some(index)) => vec![&apps[*index]],
                    (None, None) => apps.iter().collect(),
                };
                Ok(chosen.into_iter().map(App::target).collect())
            }
        }
    }

    /// The one place a command that works in one directory acts on (`rok
    /// migrate`, `rok schema diff`): as [`Project::targets`], except that a
    /// Monorepo root naming several Apps needs `--app`.
    pub fn target(&self, app: Option<&str>) -> Result<Target> {
        let mut targets = self.targets(app)?;
        if targets.len() > 1 {
            let names: Vec<&str> = targets
                .iter()
                .filter_map(|target| target.app.as_deref())
                .collect();
            bail!(
                "this Monorepo names {} Apps ({}): pass --app <name>, or run the command inside the App",
                names.len(),
                names.join(", ")
            );
        }
        targets
            .pop()
            .context("the project's `[apps]` names no App: add one, such as `api = \"apps/api\"`")
    }
}

impl App {
    fn target(&self) -> Target {
        Target {
            app: Some(self.name.clone()),
            dir: self.dir.clone(),
            settings: self.settings.clone(),
        }
    }
}

impl Target {
    /// The settings file's keys, or none without a file.
    pub(crate) fn keys(&self) -> Result<Option<FileConfig>> {
        self.settings
            .as_deref()
            .map(config_file::load_any)
            .transpose()
    }

    /// The migration history directory: the settings' `migrations`, else
    /// `migrations` in the target's directory.
    pub fn migrations_dir(&self) -> Result<PathBuf> {
        Ok(self
            .keys()?
            .and_then(|keys| keys.migrations)
            .unwrap_or_else(|| self.dir.join(DEFAULT_MIGRATIONS_DIR)))
    }

    /// The one line a command prints on stderr naming what it read: the
    /// App, its settings file and each `(what, directory)` it reads, as
    /// `using App api: apps/api/rok-data-sqlc.toml (migrations
    /// apps/api/migrations)`.
    pub fn using(&self, dirs: &[(&str, &Path)]) -> String {
        let file = self
            .settings
            .as_deref()
            .map_or_else(|| "no settings file".to_owned(), display_path);
        let source = match &self.app {
            Some(app) => format!("App {app}: {file}"),
            None => file,
        };
        using_line(&source, dirs)
    }
}

/// The report line of a command: `using <source> (<what> <dir>, …)`. A
/// command whose flag overrode the project names the flag as the source.
pub fn using_line(source: &str, dirs: &[(&str, &Path)]) -> String {
    let dirs: Vec<String> = dirs
        .iter()
        .map(|(what, dir)| format!("{what} {}", display_path(dir)))
        .collect();
    format!("using {source} ({})", dirs.join(", "))
}

/// The App `--app` names.
fn named<'a>(file: &Path, apps: &'a [App], name: &str) -> Result<&'a App> {
    apps.iter().find(|app| app.name == name).with_context(|| {
        let names: Vec<&str> = apps.iter().map(|app| app.name.as_str()).collect();
        format!(
            "--app `{name}`: {} names no such App (its `[apps]`: {})",
            display_path(file),
            names.join(", ")
        )
    })
}

/// The Apps a `rok.toml`'s `[apps]` section names, each with its directory
/// (relative to the file's) and its own settings file. `None` when the file
/// is another name or names no such section.
fn apps_of(file: &Path) -> Result<Option<Vec<App>>> {
    if file.file_name().and_then(OsStr::to_str) != Some(PROJECT_FILE) {
        return Ok(None);
    }
    let text = std::fs::read_to_string(file)
        .with_context(|| format!("failed to read {}", display_path(file)))?;
    let table: toml::Table =
        toml::from_str(&text).with_context(|| format!("invalid {}", display_path(file)))?;
    let Some(section) = table.get("apps").and_then(toml::Value::as_table) else {
        return Ok(None);
    };
    let root = file.parent().unwrap_or(Path::new(""));
    section
        .iter()
        .map(|(name, dir)| {
            let dir = dir.as_str().with_context(|| {
                format!(
                    "{}: the `apps.{name}` entry names no directory",
                    display_path(file)
                )
            })?;
            let dir = root.join(dir);
            Ok(App {
                name: name.clone(),
                settings: config_in(&dir),
                dir,
            })
        })
        .collect::<Result<Vec<_>>>()
        .map(Some)
}

/// `path` relative to `start` when it lies under it (empty for `start`
/// itself), so the paths a command reports read the way the user typed them.
fn relative(path: &Path, start: &Path) -> PathBuf {
    path.strip_prefix(start)
        .map_or_else(|_| path.to_path_buf(), Path::to_path_buf)
}
