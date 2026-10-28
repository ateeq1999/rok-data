//! The write verbs on the model (engine ADR 0018): the rendered functions
//! become associated functions of the row type — `Book::create(&db, input)`.
//! The 0.2 module wrappers (`books::write::create`), deprecated in 0.3, are
//! gone (engine ADR 0023).

/// The verbs `write.rs` puts on a model: a scope of that name would collide
/// with the associated function.
pub const MODEL_VERBS: &[&str] = &[
    "attach",
    "create",
    "delete_permanently",
    "destroy",
    "detach",
    "patch",
    "restore",
    "sync",
];

/// Wraps `items` — functions and comments rendered at column 0 — in
/// `impl {model}`.
pub fn on_the_model(model: &str, items: &str) -> String {
    if items.trim().is_empty() {
        return items.to_owned();
    }
    let mut output = format!("\nimpl {model} {{");
    for line in items.trim_start_matches('\n').lines() {
        output.push('\n');
        if !line.is_empty() {
            output.push_str("    ");
            output.push_str(line);
        }
    }
    output.push_str("\n}\n");
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    const ITEMS: &str = "
/// Inserts one row and returns it as stored.
pub async fn create<'e, E>(executor: E, input: NewBook) -> Result<Book, rok_data::Error>
where
    E: rok_data::Executor<'e>,
{
    Book::insert_builder().exec(executor).await
}

// No updatable columns detected; patch is not generated for this table.
";

    #[test]
    fn the_verbs_move_onto_the_model_with_no_module_wrappers() {
        let rendered = on_the_model("Book", ITEMS);
        assert!(rendered.starts_with(
            "\nimpl Book {\n    /// Inserts one row and returns it as stored.\n    pub async fn create<'e, E>("
        ), "{rendered}");
        assert!(
            rendered.ends_with(
                "\n    // No updatable columns detected; patch is not generated for this table.\n}\n"
            ),
            "{rendered}"
        );
        assert!(!rendered.contains("#[deprecated"), "{rendered}");
    }
}
