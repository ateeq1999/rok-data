# ADR 0021: input validation with database rules

Date: 2026-10-27 · Status: accepted

## Context

Rules such as "this email is not taken" (`unique`) and "this organization
exists" (`exists`) depend on rows in the database. The model's validation
(`#[rok(validate(…))]`, `validate_with`) is synchronous, runs inside every
insert and update, and has no session to query with. The generated APIs
already check `unique` / `exists` in their services (engine ADR 0019), but
hand-written endpoints and jobs have nothing.

Three places were possible: make the model's validation async and run the
database rules inside every write; add them to a separate validator for
input; or both.

## Decision

- **Database rules belong to input**, not to the model. A new derive
  validates any input struct: `input.validate(&session).await` consumes
  it and returns it, transforms applied, or every failure. It carries the
  field rules (one vocabulary with the model's, `#[rok(validate(…))]`),
  transforms, conditional rules, nested inputs and arrays, and the
  database rules: `unique(User::EMAIL, except = id, case_insensitive,
  scope = …)`, `exists(Org::ID, scope = …)`, and custom ones
  (`database = path`, `validate_with_database = path`).
- **They run only when asked**, never implicitly on a write. A `unique`
  check and the insert after it can race; the table's constraint is the
  guarantee. So the runtime maps a unique or foreign-key violation on a
  write to the same field error the rule reports (`Error::Validation`,
  code `unique` / `exists`), and validating inside the write's
  transaction is the recommended pattern.
- A database rule on a `#[derive(Model)]` struct fails to compile, naming
  the input derive.
- Field rules run first; database rules only on the fields that passed,
  all of one input's built-in database rules in one statement.
- One rule catalogue: the engine's generated rule implementations move
  into the runtime, errors carry dotted paths (`items.2.sku`), messages
  are customizable per rule and through a message provider, a reporter
  turns errors into the generated APIs' `422 validation_failed` body, and
  the declaration emits a JSON Schema. Generated APIs adopt the derive in
  a follow-up phase, their wire answers unchanged.

## Consequences

- A model stays a row; its writes keep their sync rules and gain the
  constraint mapping, with no hidden queries.
- `FieldError.field` becomes an owned path, and unique / foreign-key
  violations on writes change from `Error::Database` to
  `Error::Validation` (`is_unique_violation()` stays true) — breaking, in
  the unreleased section, with a migration-guide entry.
- Hand-written and generated endpoints validate, report and document
  (JSON Schema / OpenAPI) input the same way.
