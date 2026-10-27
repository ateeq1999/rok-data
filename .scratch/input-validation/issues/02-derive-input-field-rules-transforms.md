# 02: `#[derive(Input)]` with field rules and transforms

**What to build:** The `Input` derive on structs: `validate(session).await` consumes the input, applies transforms (`trim`, `lowercase`, `uppercase`, `normalize_email`, `custom = path`) in order, runs the existing field rules with the model's `#[rok(validate(…))]` spelling, and returns the input or every failure. `Option` fields optional, plain fields required; record-level `validate_with`.

**Blocked by:** 01

**Status:** done (2026-10-27) — `validate(session) -> rok_data::Result<Self>` (rule failures are `Error::Validation`), plus `validate_fields()` without a session

- [x] Derive in `rok-data-macros`, runtime trait in `rok-data-core::validate` (split into a module directory)
- [x] Settle the return type (`rok_data::Result<Self>` with `Error::Validation`) and document it
- [x] Facade tests: every transform, every existing field rule, collected failures
- [x] Prelude export; reserved-name check if any chain names are touched
