//! The steps of a `rok gen` run, in the order it takes them: each run's
//! data layer, then its migration when `--migration` is given, then a
//! Monorepo's contract export and its Clients. A step that fails stops the
//! run; nothing done is rolled back, so the run ends with what was done,
//! what failed, what was skipped and the command that finishes the rest.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::ValueEnum;
use rok_data_sqlc::clients::Project;
use serde_json::{Value, json};

use super::{Run, print_bodies_then};
use crate::cli::r#gen::GenOptions;
use crate::output::display_path;

/// One step of the run.
enum Step {
    /// A run's data layer (and API features): `app` names the App, `None`
    /// a plain project's run.
    DataLayer { app: Option<String> },
    /// A run's migration, staged as `name`; `dir` is where the run's App
    /// is, for the command that stages it alone.
    Migration {
        app: Option<String>,
        name: String,
        dir: PathBuf,
    },
    /// The contract's export command.
    Export,
    /// A Client's emission, by its key (`ts`).
    Client(&'static str),
}

impl Step {
    /// The step as the summary names it.
    fn label(&self) -> String {
        match self {
            Step::DataLayer { app: Some(app) } => format!("`{app}`'s data layer"),
            Step::DataLayer { app: None } => "the data layer".to_owned(),
            Step::Migration {
                app: Some(app),
                name,
                ..
            } => format!("`{app}`'s migration `{name}`"),
            Step::Migration {
                app: None, name, ..
            } => format!("the migration `{name}`"),
            Step::Export => "the contract export".to_owned(),
            Step::Client(key) => format!("the `{key}` client"),
        }
    }

    /// The step as one `--json` object, without its status.
    fn body(&self) -> Value {
        let mut body = match self {
            Step::DataLayer { app } => json!({ "kind": "data_layer", "app": app }),
            Step::Migration { app, name, .. } => {
                json!({ "kind": "migration", "app": app, "migration": name })
            }
            Step::Export => json!({ "kind": "export" }),
            Step::Client(key) => json!({ "kind": "client", "client": key }),
        };
        body["step"] = json!(self.label());
        body
    }
}

/// Where a step is.
enum Status {
    /// Not reached yet (and, once a step failed, skipped).
    Pending,
    Done,
    /// Failed with this error.
    Failed(String),
}

/// The run's steps, planned up front and marked as the run takes them.
pub(super) struct Steps<'a> {
    steps: Vec<(Step, Status)>,
    options: &'a GenOptions,
}

impl<'a> Steps<'a> {
    /// The steps `runs` and the Monorepo's `clients` make, in order.
    pub(super) fn plan(runs: &[Run], clients: Option<&Project>, options: &'a GenOptions) -> Self {
        let mut steps = Vec::new();
        for run in runs {
            steps.push(Step::DataLayer {
                app: run.app.clone(),
            });
            if let Some(name) = &run.settings.migration {
                steps.push(Step::Migration {
                    app: run.app.clone(),
                    name: name.clone(),
                    dir: run.dir.clone(),
                });
            }
        }
        if let Some(project) = clients {
            if !options.dry_run && project.export.is_some() {
                steps.push(Step::Export);
            }
            for client in &project.clients {
                steps.push(Step::Client(client.kind.key()));
            }
        }
        Steps {
            steps: steps
                .into_iter()
                .map(|step| (step, Status::Pending))
                .collect(),
            options,
        }
    }

    /// Marks the next step done.
    pub(super) fn done(&mut self) {
        if let Some((_, status)) = self
            .steps
            .iter_mut()
            .find(|(_, status)| matches!(status, Status::Pending))
        {
            *status = Status::Done;
        }
    }

    /// Ends the run at the next step, failed with `error`: the bodies are
    /// printed (with the steps, under `--json`), and the error carries
    /// the summary when the run had more than one step.
    pub(super) fn fail(
        mut self,
        error: String,
        mut bodies: Vec<Value>,
        multi: bool,
    ) -> Result<ExitCode, String> {
        let Some(failed) = self
            .steps
            .iter()
            .position(|(_, status)| matches!(status, Status::Pending))
        else {
            return print_bodies_then(bodies, multi, Err(error));
        };
        self.steps[failed].1 = Status::Failed(error.clone());
        if self.steps.len() < 2 {
            return print_bodies_then(bodies, multi, Err(error));
        }
        let finish = self.finish();
        if self.options.json {
            let steps: Vec<Value> = self
                .steps
                .iter()
                .map(|(step, status)| {
                    let mut body = step.body();
                    match status {
                        Status::Pending => body["status"] = json!("skipped"),
                        Status::Done => body["status"] = json!("done"),
                        Status::Failed(error) => {
                            body["status"] = json!("failed");
                            body["error"] = json!(error);
                        }
                    }
                    body
                })
                .collect();
            let summary = json!({ "steps": steps, "finish": finish.command });
            match bodies.last_mut() {
                Some(Value::Object(last)) if !multi => {
                    last.extend(summary.as_object().cloned().unwrap_or_default());
                }
                _ => bodies.push(summary),
            }
        }
        let mut message = format!(
            "{} failed: {error}\n\nsteps:\n",
            self.steps[failed].0.label()
        );
        for (step, status) in &self.steps {
            let word = match status {
                Status::Pending => "skipped",
                Status::Done => "done",
                Status::Failed(_) => "failed",
            };
            message.push_str(&format!("  {word:<8} {}\n", step.label()));
        }
        message.push_str(&format!(
            "nothing was rolled back; once the cause is fixed, `{}` finishes the rest",
            finish.command
        ));
        for (app, name, dir) in &finish.stage_alone {
            message.push_str(&format!(
                ", and `rok gen --migration {name}` in `{dir}` stages `{app}`'s"
            ));
        }
        print_bodies_then(bodies, multi, Err(message))
    }

    /// The command that finishes the run: its own flags, without the
    /// staging ones once a migration is staged (staging it again would
    /// find nothing to stage). The Apps whose migration is still to stage
    /// then stage it from their own directory.
    fn finish(&self) -> Finish {
        let staged = |status: &Status| matches!(status, Status::Done);
        let any_staged = self
            .steps
            .iter()
            .any(|(step, status)| matches!(step, Step::Migration { .. }) && staged(status));
        let stage_alone = if any_staged {
            self.steps
                .iter()
                .filter_map(|(step, status)| match step {
                    Step::Migration {
                        app: Some(app),
                        name,
                        dir,
                    } if !staged(status) => Some((app.clone(), name.clone(), display_path(dir))),
                    _ => None,
                })
                .collect()
        } else {
            Vec::new()
        };
        Finish {
            command: command(self.options, !any_staged),
            stage_alone,
        }
    }
}

/// What finishes a failed run.
struct Finish {
    /// The `rok gen` that finishes it.
    command: String,
    /// The Apps whose migration is staged alone: App, migration, dir.
    stage_alone: Vec<(String, String, String)>,
}

/// `rok gen` with the run's flags; the staging ones when `staging`.
/// `--json` and `--strict` are left out: they don't change what is done.
fn command(options: &GenOptions, staging: bool) -> String {
    let mut words = vec!["rok".to_owned(), "gen".to_owned()];
    let mut flag = |name: &str, value: Option<String>| {
        words.push(name.to_owned());
        if let Some(value) = value {
            words.push(quoted(&value));
        }
    };
    let path = |path: &PathBuf| Some(display_path(path));
    if let Some(config) = &options.config {
        flag("--config", path(config));
    }
    if let Some(schema) = &options.schema {
        flag("--schema", path(schema));
    }
    if let Some(dir) = &options.migrations_dir {
        flag("--migrations-dir", path(dir));
    }
    if let Some(output) = &options.output {
        flag("--output", path(output));
    }
    if let Some(resource) = &options.resource {
        flag("--resource", Some(resource.clone()));
    }
    for queries in &options.queries {
        flag("--queries", path(queries));
    }
    if let Some(framework) = options.api_framework
        && let Some(value) = framework.to_possible_value()
    {
        flag("--api-framework", Some(value.get_name().to_owned()));
    }
    if staging {
        if let Some(migration) = &options.migration {
            flag("--migration", Some(migration.clone()));
        }
        for rename in &options.renames {
            flag("--rename", Some(rename.clone()));
        }
        if options.allow_destructive {
            flag("--allow-destructive", None);
        }
    }
    if options.lenient {
        flag("--lenient", None);
    }
    if options.dry_run {
        flag("--dry-run", None);
    }
    if options.force {
        flag("--force", None);
    }
    words.join(" ")
}

/// A word of the command, quoted when it holds a space.
fn quoted(word: &str) -> String {
    if word.contains(char::is_whitespace) {
        format!("\"{word}\"")
    } else {
        word.to_owned()
    }
}
