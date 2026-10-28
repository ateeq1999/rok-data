# Generated validation is the runtime's `Input`

Date: 2026-10-28 · Status: accepted

Generated code validated through the `validator` crate. `derive = ["validate"]` put `validator::Validate` on create inputs and update patches (engine ADR 0015). `[api.rules]` became `validator`'s own rules or `custom` functions generated into each `dto.rs`, which called helpers in `shared/validation.rs` (engine ADR 0019). The runtime now has its own input validation (workspace ADR 0021): `#[derive(Input)]` with field rules, transforms, database rules and a catalogue that already carries every `[api.rules]` rule under the generated APIs' codes. Two validation systems in one generated app meant two error shapes, two sets of rule names, and a `validator` dependency in every app. This ADR moves generated validation onto the runtime, and it changes the generated shape that engine ADRs 0015 and 0019 describe.

**Inputs, patches and bodies derive `rok_data::Input`.**

- **The derive:** `derive = ["validate"]` puts `rok_data::Input` on `NewX` and `UpdateX`; an API's create, update and search bodies derive it too, as does the shared `IdList`.
- **The rules:** the schema's length and range checks are `#[rok(validate(length(…)))]` and `#[rok(validate(range(…)))]`. An exclusive bound is `exclusive_min` / `exclusive_max`, and a `NUMERIC` column takes `range` directly, with no generated function. A typed id implements the runtime's `HasLength` or `AsNumber`, not `validator`'s traits.
- **`[api.rules]`:** each rule is the runtime catalogue's (`alpha_numeric(dashes)`, `one_of(values = [...])`, `min_length = 3`, `mobile(locales = [...])`, …). Its code stays the VineJS name. Transforms are `#[rok(transform(trim, lowercase))]` and run before the rules, not inside `deserialize_with`. `dto.rs` has no per-column check functions, and `shared/validation.rs` no `check`, `matches_regex` or `Transformed`.
- **Custom checks with a code of their own:** `immutable`, `hidden` and `sort_key` are `custom(function = path, code = "…")`.
- **The extractor:** `ValidatedJson<T>` takes any `T: DeserializeOwned + rok_data::Input` without a context, and runs `validate_fields()`: transforms, then field rules. Database rules need a session, so they run in the service.

**What changes on the wire, and in a project.**

- **`params`:** a failure's `params` no longer echo the rejected `value`. An exact length is `{"min": n, "max": n}`, not `{"equal": n}`.
- **Dependencies:** an app drops `validator` and the `regex` crate, and enables rok-data's `regex` feature when a rule matches a pattern (`regex`, `mobile`, `postalCode`). The data layer's dependency hints say so.
- **Hand-written request types:** a type that derived `validator::Validate` to go through `ValidatedJson` derives `rok_data::Input` instead. The migration guide lists the attribute changes.

## Considered Options

- **Keep `validator` in generated code, and convert its errors at the extractor.** This was the first step of the switch (`from_validator`). It left two rule vocabularies, two sets of trait impls on typed ids, and generated check functions duplicating the runtime's catalogue.
- **Generate rules through `validator`, with the runtime's catalogue as its custom functions.** This cuts the duplication, but the generated app still needs `validator`, and hand-written inputs still have two derives to choose from.
- **Switch fully to `Input` (chosen).** One derive, one rule vocabulary, one error type from the extractor to the database rules.

## Consequences

- A generated body's rules read like a hand-written input's. The generated `dto.rs` is shorter: it has no check functions and no transform deserializers.
- `ValidatedJson` checks field rules only. A body's database rules (`unique`, `exists`) stay the service's pre-checks (engine ADR 0019), now reported through the same `ValidationErrors`.
- rustfmt formats an attribute only when every value is a literal. A rule with a path or an array value (`custom(function = …)`, `one_of(values = [...])`) is left on one line as written. The generator lays out the others as rustfmt would (`generator/rok_attr.rs`).
