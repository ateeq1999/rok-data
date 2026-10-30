//! Terminal output helpers and the small pieces of the JSON contract.

use std::path::Path;

use crate::config::ServerConfig;
use serde_json::{Value, json};

/// A path the way the CLI prints it: relative to the current directory when
/// it is below it, with `/` as the separator on every platform.
pub(crate) fn display_path(path: &Path) -> String {
    let current = std::env::current_dir().ok();
    let shown = current
        .as_deref()
        .and_then(|current| path.strip_prefix(current).ok())
        .filter(|relative| !relative.as_os_str().is_empty())
        .unwrap_or(path);
    let mut text = shown.display().to_string();
    if std::path::MAIN_SEPARATOR == '\\' {
        text = text.replace('\\', "/");
    }
    while let Some(rest) = text.strip_prefix("./") {
        text = rest.to_owned();
    }
    text
}

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

/// JSON output for `rok server list --json`.
pub(crate) fn server_list_json(
    servers: &[ServerConfig],
    password_statuses: &[PasswordStatus],
) -> Value {
    let servers_json: Vec<Value> = servers
        .iter()
        .zip(password_statuses.iter())
        .map(|(server, status)| {
            json!({
                "name": server.name,
                "host": server.host,
                "port": server.port,
                "user": server.user,
                "database": server.database,
                "sslmode": server.sslmode,
                "password": status.to_json(),
            })
        })
        .collect();
    json!({ "servers": servers_json })
}

/// JSON output for `rok server add --json`.
pub(crate) fn server_add_json(name: &str, keychain_available: bool) -> Value {
    json!({
        "name": name,
        "added": true,
        "password_stored_in_keychain": keychain_available,
    })
}

/// JSON output for `rok server remove --json`.
pub(crate) fn server_remove_json(name: &str, keychain_available: bool) -> Value {
    json!({
        "name": name,
        "removed": true,
        "password_deleted_from_keychain": keychain_available,
    })
}

/// Password status for a server in the list output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PasswordStatus {
    Keychain,
    Env,
    Missing,
}

impl PasswordStatus {
    fn to_json(self) -> Value {
        match self {
            PasswordStatus::Keychain => json!({ "source": "keychain", "present": true }),
            PasswordStatus::Env => json!({ "source": "env", "present": true }),
            PasswordStatus::Missing => json!({ "source": "none", "present": false }),
        }
    }
}

/// Standard machine-readable `ok`/`error` JSON footer used by `--json`
/// commands: everything the success/exit behavior reports.
pub(crate) fn json_line(value: Value) {
    println!("{value}");
}
