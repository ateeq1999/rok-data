//! Shared plumbing for the `rok db …` and `rok server …` binary suites.
//!
//! Every command runs with `ROK_CONFIG_DIR` in a temporary directory and
//! `ROK_KEYCHAIN=off`, so no test reads or writes the user's config or OS
//! keychain; passwords travel as `ROK_SERVER_<NAME>_PASSWORD`. The
//! inherited `DATABASE_URL` and `ROK_SERVER_*` variables are removed, so a
//! test sets exactly the ones it means.
//!
//! The DB-backed tests read `DATABASE_URL` and skip with a printed note when
//! it is unset. They only ever create and drop databases of their own
//! (`rok_t_…`), never the one `DATABASE_URL` names.

use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

use tempfile::TempDir;

/// A sandbox for one test: its own config directory.
pub(crate) struct Sandbox {
    dir: TempDir,
}

impl Sandbox {
    pub(crate) fn new() -> Self {
        Sandbox {
            dir: TempDir::new().expect("temp dir"),
        }
    }

    /// `rok <args>` in the sandbox, with no inherited database settings.
    pub(crate) fn rok(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_rok"));
        cmd.args(args)
            .env("ROK_CONFIG_DIR", self.dir.path().join("config"))
            .env("ROK_KEYCHAIN", "off")
            .env_remove("DATABASE_URL");
        for (name, _) in std::env::vars() {
            if name.starts_with("ROK_SERVER_") {
                cmd.env_remove(name);
            }
        }
        cmd
    }

    /// Runs `rok <args>` and returns its output.
    pub(crate) fn run(&self, args: &[&str]) -> Output {
        self.rok(args).output().expect("spawn rok")
    }

    /// Registers `name` with `url` (`rok server add NAME --url URL`).
    pub(crate) fn add_server(&self, name: &str, url: &str) {
        assert_success(&self.run(&["server", "add", name, "--url", url]));
    }
}

/// `DATABASE_URL`, or `None` after printing why the caller skips.
pub(crate) fn database_url_or_skip(test: &str) -> Option<url::Url> {
    match std::env::var("DATABASE_URL").ok().filter(|u| !u.is_empty()) {
        Some(url) => Some(url::Url::parse(&url).expect("DATABASE_URL is a URL")),
        None => {
            eprintln!("skipping {test}: DATABASE_URL not set");
            None
        }
    }
}

/// `base` pointed at database `name`: only the path changes, so user,
/// password, host and port stay as `DATABASE_URL` gives them.
pub(crate) fn url_for(base: &url::Url, name: &str) -> String {
    let mut url = base.clone();
    url.set_path(&format!("/{name}"));
    url.to_string()
}

/// A database name no other test (or run) uses.
pub(crate) fn unique_name(tag: &str) -> String {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    format!(
        "rok_t_{tag}_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

/// A throwaway database, created through `rok db create` on the
/// `postgres` maintenance database and dropped again when it goes out of
/// scope.
pub(crate) struct TestDb {
    pub(crate) name: String,
    pub(crate) url: String,
    admin_url: String,
}

impl TestDb {
    pub(crate) fn create(base: &url::Url, tag: &str) -> Self {
        let name = unique_name(tag);
        let admin_url = url_for(base, "postgres");
        let output = Sandbox::new().run(&["db", "create", &name, "--database-url", &admin_url]);
        assert_success(&output);
        TestDb {
            url: url_for(base, &name),
            name,
            admin_url,
        }
    }
}

impl Drop for TestDb {
    fn drop(&mut self) {
        let output = Sandbox::new().run(&[
            "db",
            "drop",
            &self.name,
            "--database-url",
            &self.admin_url,
            "--yes",
        ]);
        if !output.status.success() {
            eprintln!(
                "cleanup: could not drop {}: {}",
                self.name,
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

/// The command's stdout parsed as one JSON value.
pub(crate) fn json(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout is not JSON ({e}):\n{}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

pub(crate) fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

pub(crate) fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Asserts the command succeeded, showing both streams when it did not.
pub(crate) fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed ({:?}):\nstdout: {}\nstderr: {}",
        output.status.code(),
        stdout(output),
        stderr(output)
    );
}

/// Asserts the command failed with exit code 1 and `message` on stderr.
pub(crate) fn assert_error(output: &Output, message: &str) {
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit 1:\nstdout: {}\nstderr: {}",
        stdout(output),
        stderr(output)
    );
    assert!(
        stderr(output).contains(message),
        "expected {message:?} on stderr:\n{}",
        stderr(output)
    );
}

/// Whether database `name` exists, asked of the server through `rok db query`.
pub(crate) fn database_exists(base: &url::Url, name: &str) -> bool {
    let output = Sandbox::new().run(&[
        "db",
        "query",
        &format!("SELECT count(*) AS n FROM pg_database WHERE datname = '{name}'"),
        "--database-url",
        &url_for(base, "postgres"),
        "--json",
    ]);
    assert_success(&output);
    let report = json(&output);
    report["results"][0]["rows"][0][0] == "1"
}
