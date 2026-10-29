//! `rok sqlc init`: scaffolds the application skeleton of engine ADR 0016 into a
//! project, each file only when absent. It generates nothing; `rok sqlc generate`
//! does that once the truth exists.
//!
//! The application is a library crate (`src/lib.rs` declares every module
//! `pub`) with a thin `src/main.rs` that starts it, so generated code the
//! application does not use yet never warns. The skeleton refers to paths
//! that generation fills in: `crate::features::{register_services,
//! configure_all, GeneratedApiDoc}` from the generated
//! `features/generated.rs`,
//! `crate::shared::{health, pagination, response, tenant, validation}`, and
//! `crate::db::pool::{PoolSettings, init_pool}`.
//!
//! `init --template <NAME>` ([`init_template`]) copies a ready-made project
//! of the `rok-templates` crate instead.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::ApiFramework;
use crate::output::{SCAFFOLD_HEADER, write_absent};
use crate::report::display_path;

/// The skeleton's files every framework shares: each file's path in the
/// project and its text. Rust files get [`SCAFFOLD_HEADER`] as their first
/// line; the others carry it in their own comment syntax.
const COMMON: &[(&str, &str)] = &[
    ("AGENTS.md", include_str!("scaffold/AGENTS.md")),
    ("src/config.rs", include_str!("scaffold/config.rs")),
    ("src/openapi.rs", include_str!("scaffold/openapi.rs")),
    ("src/shared/mod.rs", include_str!("scaffold/shared/mod.rs")),
];

/// The actix-web skeleton's own files: its wiring, extractors and config.
const ACTIX_WEB: &[(&str, &str)] = &[
    (
        "rok-data-sqlc.toml",
        include_str!("scaffold/rok-data-sqlc.toml"),
    ),
    ("src/app.rs", include_str!("scaffold/app.rs")),
    ("src/error.rs", include_str!("scaffold/error.rs")),
    (
        "src/features/mod.rs",
        include_str!("scaffold/features/mod.rs"),
    ),
    ("src/lib.rs", include_str!("scaffold/lib.rs")),
    ("src/main.rs", include_str!("scaffold/main.rs")),
    (
        "src/shared/auth.rs",
        include_str!("scaffold/shared/auth.rs"),
    ),
];

/// The axum skeleton's own files (engine ADR 0021): the same paths, with
/// its `router(pool)` wiring and axum's extractors.
const AXUM: &[(&str, &str)] = &[
    (
        "rok-data-sqlc.toml",
        include_str!("scaffold/axum/rok-data-sqlc.toml"),
    ),
    ("src/app.rs", include_str!("scaffold/axum/app.rs")),
    ("src/error.rs", include_str!("scaffold/axum/error.rs")),
    (
        "src/features/mod.rs",
        include_str!("scaffold/axum/features/mod.rs"),
    ),
    ("src/lib.rs", include_str!("scaffold/axum/lib.rs")),
    ("src/main.rs", include_str!("scaffold/axum/main.rs")),
    (
        "src/shared/auth.rs",
        include_str!("scaffold/axum/shared/auth.rs"),
    ),
];

/// The truth dir the schema files go in, created empty. The migration
/// history (`migrations/` by default) appears when the first pair stages.
const SCHEMA: &str = "schema";

/// Where `main.rs` names the library crate.
const CRATE_PLACEHOLDER: &str = "{crate}";

/// Where `AGENTS.md` lists the rules `check-arch` enforces.
const RULES_PLACEHOLDER: &str = "{rules}";

/// The skeleton's files under `dir`, with their contents, for the library
/// crate `krate` and `framework`. Line endings are `\n` whichever way the
/// templates were checked out.
pub(crate) fn plan(dir: &Path, krate: &str, framework: ApiFramework) -> Vec<(PathBuf, String)> {
    let own = match framework {
        ApiFramework::ActixWeb => ACTIX_WEB,
        ApiFramework::Axum => AXUM,
    };
    COMMON
        .iter()
        .chain(own)
        .map(|(path, text)| {
            let text = text
                .replace("\r\n", "\n")
                .replace(CRATE_PLACEHOLDER, krate)
                // Rule 1 names the framework the skeleton is written for.
                .replace(
                    RULES_PLACEHOLDER,
                    &crate::check_arch::rules_markdown(framework),
                );
            let contents = if path.ends_with(".rs") {
                format!("{SCAFFOLD_HEADER}\n\n{text}")
            } else {
                text
            };
            (dir.join(path), contents)
        })
        .collect()
}

/// The library crate's name in `dir`: `Cargo.toml`'s `[lib] name`, else its
/// `[package] name` with `-` as `_`, else the directory's name made an
/// identifier (`app` when it cannot be).
fn crate_name(dir: &Path) -> String {
    let declared = fs::read_to_string(dir.join("Cargo.toml"))
        .ok()
        .and_then(|text| declared_crate(&text));
    if let Some(name) = declared {
        return name;
    }
    let name: String = fs::canonicalize(dir)
        .ok()
        .and_then(|dir| dir.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    if name.starts_with(|c: char| c.is_ascii_alphabetic()) {
        name
    } else {
        "app".to_owned()
    }
}

/// The library crate a `Cargo.toml` declares: its `[lib] name`, else its
/// `[package] name` with `-` as `_`.
fn declared_crate(manifest: &str) -> Option<String> {
    let manifest = manifest.parse::<toml::Table>().ok()?;
    ["lib", "package"].iter().find_map(|section| {
        manifest
            .get(*section)?
            .get("name")?
            .as_str()
            .map(|name| name.replace('-', "_"))
    })
}

/// What `init` did.
#[derive(Debug, Default)]
pub struct Scaffolded {
    /// The files written.
    pub written: Vec<PathBuf>,
    /// Whether `src/main.rs` was cargo's untouched default and was replaced.
    pub replaced_default_main: bool,
    /// Whether a `Cargo.toml` was written: `init --template` copies the
    /// template's when the project has none.
    pub wrote_manifest: bool,
}

/// The `src/main.rs` `cargo new` writes. It is not the user's code, so the
/// scaffolded `main.rs` replaces it.
const CARGO_DEFAULT_MAIN: &str = "fn main() {\n    println!(\"Hello, world!\");\n}";

/// Whether `text` is [`CARGO_DEFAULT_MAIN`], whatever its line endings and
/// trailing newline.
fn is_cargo_default_main(text: &str) -> bool {
    text.replace("\r\n", "\n").trim_end_matches('\n') == CARGO_DEFAULT_MAIN
}

/// Scaffolds `framework`'s skeleton into `dir`, keeping every file that
/// exists but cargo's untouched default `src/main.rs`, and creates
/// `schema/`.
pub fn init(dir: &Path, framework: ApiFramework) -> Result<Scaffolded> {
    let truth = dir.join(SCHEMA);
    fs::create_dir_all(&truth)
        .with_context(|| format!("failed to create directory `{}`", display_path(&truth)))?;
    let main = dir.join("src/main.rs");
    let replaced_default_main =
        fs::read_to_string(&main).is_ok_and(|text| is_cargo_default_main(&text));
    if replaced_default_main {
        fs::remove_file(&main)
            .with_context(|| format!("failed to replace `{}`", display_path(&main)))?;
    }
    Ok(Scaffolded {
        written: write_absent(&plan(dir, &crate_name(dir), framework))?,
        replaced_default_main,
        wrote_manifest: false,
    })
}

mod template;

pub use template::{init_template, names as template_names};

#[cfg(test)]
mod tests;
