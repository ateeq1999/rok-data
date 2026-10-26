//! The Rust samples of the workspace docs (`docs/*.md`), compiled.
//!
//! Each sample is marked in its document by `<!-- source: <path> -->`, the
//! path of a compiled file holding the same text (at any indentation), or
//! by `<!-- not compiled: <reason> -->` for code that is not this release's
//! (a migration's "before"). The test below holds the documents to that.

#![allow(dead_code, unused_variables)]

mod getting_started;
mod migration_guide;
mod pagination;

use std::path::{Path, PathBuf};

/// The repository root.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// One fenced Rust block: where it starts, the marker on the line above it,
/// and its text.
struct Block {
    line: usize,
    marker: Option<String>,
    body: String,
}

/// The Rust blocks of `document`.
fn rust_blocks(document: &str) -> Vec<Block> {
    let lines: Vec<&str> = document.lines().collect();
    let mut blocks = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].starts_with("```rust") {
            let marker = i
                .checked_sub(1)
                .map(|above| lines[above].trim())
                .and_then(|above| above.strip_prefix("<!--"))
                .and_then(|above| above.strip_suffix("-->"))
                .map(|marker| marker.trim().to_owned());
            let start = i;
            let mut body = String::new();
            i += 1;
            while i < lines.len() && !lines[i].starts_with("```") {
                body.push_str(lines[i]);
                body.push('\n');
                i += 1;
            }
            blocks.push(Block {
                line: start + 1,
                marker,
                body,
            });
        }
        i += 1;
    }
    blocks
}

/// `true` when `source` holds `body` indented by some multiple of four.
fn holds(source: &str, body: &str) -> bool {
    (0..5).any(|level| {
        let pad = " ".repeat(level * 4);
        let indented: String = body
            .lines()
            .map(|line| {
                if line.is_empty() {
                    "\n".to_owned()
                } else {
                    format!("{pad}{line}\n")
                }
            })
            .collect();
        source.contains(&indented)
    })
}

#[test]
fn every_rust_sample_compiles_or_says_why_not() {
    let mut problems = Vec::new();
    for name in [
        "docs/getting-started.md",
        "docs/migration-guide.md",
        "docs/pagination.md",
    ] {
        let document = std::fs::read_to_string(root().join(name))
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        for block in rust_blocks(&document) {
            let at = format!("{name}:{}", block.line);
            match block.marker.as_deref() {
                Some(marker) if marker.starts_with("source:") => {
                    let path = marker["source:".len()..].trim();
                    match std::fs::read_to_string(root().join(path)) {
                        Ok(source) if holds(&source, &block.body) => {}
                        Ok(_) => problems.push(format!(
                            "{at}: the block is not in {path}:\n{}",
                            block.body
                        )),
                        Err(error) => problems.push(format!("{at}: {path}: {error}")),
                    }
                }
                Some(marker)
                    if marker
                        .strip_prefix("not compiled:")
                        .is_some_and(|why| !why.trim().is_empty()) => {}
                _ => problems.push(format!(
                    "{at}: a Rust block needs `<!-- source: <path> -->` or `<!-- not compiled: <reason> -->` on the line above"
                )),
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n\n"));
}
