//! What the command line prints: a run's [`Report`], and `init`'s files and
//! next steps.

use crate::report::display_path;
use crate::{Config, Report};

/// Prints what `init` wrote.
pub(super) fn print_scaffolded(scaffolded: crate::init::Scaffolded) {
    if scaffolded.replaced_default_main {
        println!("replaced cargo's default src/main.rs");
    }
    for path in scaffolded.written {
        println!("scaffolded {}", display_path(&path));
    }
}

/// Prints `init`'s numbered next steps.
pub(super) fn print_next_steps(steps: &[&str]) {
    println!("\nNext steps:");
    for (number, step) in steps.iter().enumerate() {
        println!("  {}. {step}", number + 1);
    }
}

/// Prints what a run did: skipped or fatal tables and queries, and the
/// warnings, on stderr; planned or written files on stdout.
pub(super) fn print_report(config: &Config, report: &Report) {
    for failure in &report.table_failures {
        if config.lenient {
            eprintln!(
                "warning: {}table `{}` skipped: {}",
                failure.location(),
                failure.table,
                failure.reason
            );
        } else {
            eprintln!("error: {failure}");
        }
    }
    for failure in &report.query_failures {
        if config.lenient {
            eprintln!(
                "warning: {}query `{}` skipped: {}",
                failure.location(),
                failure.query,
                failure.reason
            );
        } else {
            eprintln!("error: {failure}");
        }
    }
    for warning in &report.warnings {
        eprintln!("warning: {warning}");
    }
    for path in &report.written {
        if config.dry_run {
            println!("would write {}", display_path(path));
        } else {
            println!("generated {}", display_path(path));
        }
    }
    for path in &report.scaffolded {
        if config.dry_run {
            println!("would scaffold {}", display_path(path));
        } else {
            println!("scaffolded {}", display_path(path));
        }
    }
    for path in &report.unchanged {
        println!("unchanged {}", display_path(path));
    }
    for path in &report.missing {
        println!("missing {}", display_path(path));
    }
    for path in &report.changed {
        println!("changed {}", display_path(path));
    }
    for path in &report.stale {
        if config.check {
            println!("stale {}", display_path(path));
        } else if config.dry_run {
            println!("would remove {}", display_path(path));
        } else {
            println!("removed {}", display_path(path));
        }
    }
    for refused in &report.refused {
        let (path, reason) = (display_path(&refused.path), &refused.reason);
        if config.dry_run {
            println!("would refuse {path} ({reason})");
        } else {
            eprintln!("error: refused to write {path} ({reason})");
        }
    }
}
