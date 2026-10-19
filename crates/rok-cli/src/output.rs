//! Terminal output helpers and the small pieces of the JSON contract.

use serde_json::{Value, json};

/// Turns `StatusRow` rows into the JSON body of `rok migrate status --json`.
pub(crate) fn status_json(rows: &[rok_data::migration_file::StatusRow]) -> Value {
    let migrations: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "version": row.version,
                "name": row.name,
                "state": row.state,
                "checksum": row.checksum,
                "drift": row.drift,
            })
        })
        .collect();
    let drift = rows.iter().any(|row| row.drift);
    json!({ "migrations": migrations, "drift": drift })
}

/// Standard machine-readable `ok`/`error` JSON footer used by `--json`
/// commands: everything the success/exit behavior reports.
pub(crate) fn json_line(value: Value) {
    println!("{value}");
}
