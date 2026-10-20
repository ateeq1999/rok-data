//! The stable library entry point, `rok_data_sqlc::generate`, called the way a
//! `build.rs` would call it.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use rok_data_sqlc::Config;
use tempfile::TempDir;

mod api;
mod extractors;
mod generation;

const SCHEMA: &str = "CREATE TABLE users (id UUID PRIMARY KEY, email TEXT NOT NULL);\n\
     CREATE TABLE spans (id BIGSERIAL PRIMARY KEY, duration POINT NOT NULL);";

const QUERIES: &str = "-- name: GetUserByEmail :maybe_one
SELECT id, email FROM users WHERE email = $1;

-- name: Broken :one
SELECT id FROM no_such_table WHERE id = $1;
";

struct Inputs {
    dir: TempDir,
}

impl Inputs {
    fn new() -> Result<Self> {
        let dir = TempDir::new()?;
        fs::write(dir.path().join("schema.sql"), SCHEMA)?;
        fs::write(dir.path().join("queries.sql"), QUERIES)?;
        Ok(Self { dir })
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    fn config(&self, output: &str) -> Config {
        let mut config = Config::default();
        config.schema = vec![self.path("schema.sql")];
        config.queries = vec![self.path("queries.sql")];
        config.output = self.path(output);
        config.lenient = true;
        config
    }
}

/// Every file under `root`, keyed by its path relative to `root`.
fn tree(root: &Path) -> Result<BTreeMap<PathBuf, String>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, String>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                walk(root, &path, out)?;
            } else {
                out.insert(
                    path.strip_prefix(root)?.to_path_buf(),
                    fs::read_to_string(&path)?,
                );
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out)?;
    Ok(out)
}
