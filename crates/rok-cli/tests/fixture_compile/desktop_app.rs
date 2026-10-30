//! The `apps/desktop` App of the monorepo template (workspace ADR 0031),
//! made the way a user makes it: `rok init --template monorepo` into a
//! scratch directory, then, from its root, `cargo build -p desktop` and
//! `cargo test -p desktop` (the App's bloc tests; GPUI's test support, no
//! window). Skips with a printed note on Linux without the system
//! libraries GPUI builds against.

use std::process::Command;

use anyhow::{Result, ensure};

use crate::common::{self, cargo};
use crate::rok_data_sqlc;

/// The Debian packages that give GPUI's Linux build what it links against
/// (what the `desktop` CI job installs).
const LINUX_PACKAGES: &str = "libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev \
     libvulkan-dev libfontconfig-dev libx11-xcb-dev libxcb1-dev";

/// The pkg-config names of the libraries GPUI's Linux build needs.
const LINUX_LIBRARIES: &[&str] = &["xkbcommon", "xkbcommon-x11", "wayland-client", "fontconfig"];

/// Why GPUI cannot build here, or `None` when it can: on Linux, a library
/// `pkg-config` cannot find (or no `pkg-config` at all). Windows and macOS
/// need nothing beyond the toolchain.
fn gpui_skip_reason() -> Option<String> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let missing: Vec<&str> = LINUX_LIBRARIES
        .iter()
        .copied()
        .filter(|library| {
            !Command::new("pkg-config")
                .args(["--exists", library])
                .status()
                .is_ok_and(|status| status.success())
        })
        .collect();
    (!missing.is_empty()).then(|| {
        format!(
            "GPUI's Linux libraries are missing ({}, per pkg-config); \
             `sudo apt-get install {LINUX_PACKAGES}` installs them",
            missing.join(", ")
        )
    })
}

/// The gate: scaffold the monorepo, then build the desktop App and run its
/// tests from the workspace root.
#[test]
fn monorepo_desktop_app_builds_and_tests() -> Result<()> {
    if let Some(reason) = gpui_skip_reason() {
        eprintln!("note: skipping the desktop App gate: {reason}");
        return Ok(());
    }
    let project = tempfile::TempDir::new()?;
    let root = project.path();
    rok_data_sqlc(root, &["init", "--template", "monorepo"])?;
    ensure!(
        root.join("apps/desktop/Cargo.toml").is_file(),
        "the scaffold has no apps/desktop/Cargo.toml"
    );
    // The root `Cargo.lock` the template ships covers the desktop App:
    // `--locked` fails if resolving it would change the lock.
    for args in [
        ["build", "-p", "desktop", "--locked"],
        ["test", "-p", "desktop", "--locked"],
    ] {
        let status = cargo(root, &args).status()?;
        ensure!(
            status.success(),
            "`cargo {}` in {} failed: {status}",
            args.join(" "),
            common::shown(root)
        );
    }
    Ok(())
}
