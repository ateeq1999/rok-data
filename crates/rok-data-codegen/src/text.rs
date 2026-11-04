//! Line endings: a CRLF checkout is not a schema change.
//!
//! The diff compares SQL text exactly (a function's body rides into an
//! `Extra` as written), so every text the staging compares — the truth
//! files and the snapshot's stored SQL — is read with CRLF line endings
//! normalised to `\n`, as the engine reads its schema and query files.

use std::fs;
use std::io;
use std::path::Path;

use serde::de::DeserializeOwned;
use serde_json::Value;

/// `text` with every CRLF line ending turned into `\n`.
pub(crate) fn lf(text: String) -> String {
    if text.contains("\r\n") {
        text.replace("\r\n", "\n")
    } else {
        text
    }
}

/// Reads a text file with CRLF line endings normalised to `\n`.
pub(crate) fn read(path: &Path) -> io::Result<String> {
    fs::read_to_string(path).map(lf)
}

/// Parses JSON whose strings have their CRLF line endings normalised to
/// `\n`: a snapshot an older staging recorded from a CRLF file compares
/// equal to the LF text.
pub(crate) fn from_json<T: DeserializeOwned>(json: &str) -> serde_json::Result<T> {
    let mut value: Value = serde_json::from_str(json)?;
    normalise(&mut value);
    serde_json::from_value(value)
}

fn normalise(value: &mut Value) {
    match value {
        Value::String(text) => *text = lf(std::mem::take(text)),
        Value::Array(items) => items.iter_mut().for_each(normalise),
        Value::Object(fields) => fields.values_mut().for_each(normalise),
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}
