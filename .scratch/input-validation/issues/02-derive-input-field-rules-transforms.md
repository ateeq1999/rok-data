# 02: `#[derive(Input)]` with field rules and transforms

**What to build:** The `Input` derive on structs: `validate(session).await` consumes the input, applies transforms (`trim`, `lowercase`, `uppercase`, `normalize_email`, `custom = path`) in order, runs the existing field rules with the model's `#[rok(validate(…))]` spelling, and returns the input or every failure. `Option` fields optional, plain fields required; record-level `validate_with`.

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] Derive in `rok-data-macros`, runtime trait in `rok-data-core::validate` (split into a module directory)
- [ ] Settle the return type (`rok_data::Result<Self>` with `Error::Validation`) and document it
- [ ] Facade tests: every transform, every existing field rule, collected failures
- [ ] Prelude export; reserved-name check if any chain names are touched
