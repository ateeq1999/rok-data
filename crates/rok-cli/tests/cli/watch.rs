//! `rok sqlc generate --watch`: generates, then regenerates on every change
//! to the schema, the query files or the config file.

use std::io::{BufRead, BufReader};
use std::process::{Child, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use super::*;

/// A running watch, killed when dropped.
struct Watch {
    child: Child,
    lines: Receiver<String>,
}

impl Watch {
    fn start(dir: &Path) -> Result<Self> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_rok"))
            .current_dir(dir)
            .args(["sqlc", "g", "--watch"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("no stdout"))?;
        let (sender, lines) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout)
                .lines()
                .map_while(std::io::Result::ok)
            {
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        Ok(Self { child, lines })
    }

    /// The first line containing `text`, failing after 60 seconds.
    fn wait_for(&self, text: &str) -> Result<String> {
        let mut seen = Vec::new();
        loop {
            match self.lines.recv_timeout(Duration::from_secs(60)) {
                Ok(line) if line.contains(text) => return Ok(line),
                Ok(line) => seen.push(line),
                Err(_) => anyhow::bail!("no line with `{text}`; saw {seen:#?}"),
            }
        }
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn a_watch_regenerates_on_change_and_survives_a_failure() -> Result<()> {
    let dir = TempDir::new()?;
    fs::write(dir.path().join("schema.sql"), SIMPLE_SCHEMA)?;
    fs::create_dir(dir.path().join("queries"))?;
    let queries = dir.path().join("queries").join("users.sql");
    fs::write(
        &queries,
        "-- name: CountUsers :one\nSELECT count(*) AS total FROM users;\n",
    )?;
    fs::write(
        dir.path().join("rok-data-sqlc.toml"),
        "schema = [\"schema.sql\"]\nqueries = [\"queries\"]\noutput = \"out\"\n",
    )?;
    let generated = dir.path().join("out").join("queries").join("users.rs");

    let watch = Watch::start(dir.path())?;
    let watching = watch.wait_for("watching")?;
    assert!(watching.contains("schema.sql"), "{watching}");
    assert!(fs::read_to_string(&generated)?.contains("count_users"));

    // A query added: one line, and the function is there.
    fs::write(
        &queries,
        "-- name: CountUsers :one\nSELECT count(*) AS total FROM users;\n\n\
         -- name: AllEmails :many\nSELECT email FROM users;\n",
    )?;
    let line = watch.wait_for("regenerated")?;
    assert!(line.contains("written"), "{line}");
    assert!(fs::read_to_string(&generated)?.contains("all_emails"));

    // A query that doesn't analyze fails the run, not the watch.
    fs::write(&queries, "-- name: Broken :one\nSELECT nope FROM users;\n")?;
    watch.wait_for("generation failed; watching for changes")?;
    fs::write(
        &queries,
        "-- name: CountUsers :one\nSELECT count(*) AS total FROM users;\n",
    )?;
    watch.wait_for("regenerated")?;
    assert!(!fs::read_to_string(&generated)?.contains("all_emails"));
    Ok(())
}

#[test]
fn watch_does_not_combine_with_check_or_dry_run() {
    for flag in ["--check", "--dry-run"] {
        let output = run_binary(&["generate", "--watch", flag, "--schema", "schema.sql"]);
        assert!(!output.status.success(), "{flag}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("cannot be used with"), "{flag}: {stderr}");
    }
}
