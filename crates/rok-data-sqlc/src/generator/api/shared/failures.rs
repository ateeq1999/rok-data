//! `FieldFailures`, appended to `shared/validation.rs` for either
//! framework (engine ADR 0019): it collects the field failures of a write
//! into one `422`, in the `details` shape of the body's rules.

/// The items `validation.rs` gains. Written with `validator`'s paths in
/// full, so neither framework's template needs another import.
pub(super) const FIELD_FAILURES: &str = r#"
/// The field failures a write's pre-checks find, such as a foreign key
/// naming no row the caller can see (`exists`) or a value another row
/// already has (`unique`). A generated `create_in` or `update_in` runs
/// every pre-check on its transaction, adding each failure here, then
/// calls [`into_result`](Self::into_result) before it writes, so one
/// `422 validation_failed` lists them all, in the `details` shape of the
/// body's rules.
#[derive(Debug, Default)]
pub struct FieldFailures(validator::ValidationErrors);

impl FieldFailures {
    /// Adds the failure `code` of `field`, with `message`.
    pub fn add(&mut self, field: &'static str, code: &'static str, message: String) {
        let error = validator::ValidationError::new(code).with_message(message.into());
        self.0.add(field, error);
    }

    /// Adds the `exists` failure of `field`, which names no `singular` row
    /// the caller can see.
    pub fn missing(&mut self, field: &'static str, singular: &str) {
        let message = format!("{field} does not refer to an existing {singular}");
        self.add(field, "exists", message);
    }

    /// Adds the `unique` failure of `field`, whose value another row
    /// already has.
    pub fn taken(&mut self, field: &'static str) {
        self.add(field, "unique", format!("{field} is already taken"));
    }

    /// Adds the `unique` failure of a composite key, whose values together
    /// another row already has, against its last field.
    pub fn taken_together(&mut self, fields: &[&'static str]) {
        if let [init @ .., last] = fields {
            let names = if init.is_empty() {
                (*last).to_owned()
            } else {
                format!("{} and {last}", init.join(", "))
            };
            let message = format!("{names} are already taken together");
            self.add(last, "unique", message);
        }
    }

    /// `Ok` when no failure was added.
    ///
    /// # Errors
    ///
    /// The `422 validation_failed` listing every failure added.
    pub fn into_result(self) -> Result<(), AppError> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(AppError::from(self.0))
        }
    }
}
"#;
