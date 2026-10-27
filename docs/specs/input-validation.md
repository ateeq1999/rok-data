# Spec — input validation with database rules

Status: ready-for-agent · Tickets: `.scratch/input-validation/issues` · Decided: 2026-10-27 (grilling session) · Implements: workspace ADR 0021 · Glossary: **Input**, **Field rule**, **Database rule** (`GLOSSARY.md`)

---

## Problem Statement

A developer validating a request body, a form or a job's payload can check
a value's shape (the model's `#[rok(validate(…))]` field rules), but not
what depends on rows in the database: "this email is not taken", "this
organization exists". The generated APIs do these checks in generated
service code (engine ADR 0019); hand-written endpoints have nothing, and
the two kinds of endpoint validate, report and document input differently.

The model's validation is synchronous and runs inside every write, with no
session to query; running queries there would hide reads in every
`insert` and still race with concurrent writers.

## Solution

A derive for **input** — untrusted data on its way to becoming rows —
whose `validate(&session).await` runs field rules, transforms,
conditional rules, nested inputs and **database rules**, and returns the
input (transformed) or every failure with a dotted path. Database rules
run only when asked; the table's constraints stay the guarantee, and a
constraint violation on a write reports the same field error the rule
would. One rule catalogue serves models, inputs and the generated APIs,
which adopt the derive in the last phase with their wire answers
unchanged.

## User Stories

1. As a developer, I want `#[derive(Input)]` on any struct and `input.validate(&db).await?`, so that a request body is checked before it becomes rows.
2. As a developer, I want the field rules I know from the model (`length`, `range`, `email`, `non_empty`, `custom`) on inputs with the same `#[rok(validate(…))]` spelling, so that a rule moves between a model and an input unchanged.
3. As a developer, I want `unique(User::EMAIL)` on a field, so that a taken value is a field error before the insert.
4. As a developer updating a row, I want `unique(User::EMAIL, except = id)`, so that the row being updated doesn't collide with itself.
5. As a developer, I want `unique(…, case_insensitive)` and `unique(…, scope = path)` (`fn(&Self) -> Expr<Model>`), so that a case-insensitive index or a tenant is respected.
6. As a developer, I want `exists(Org::ID)` (with `scope`), so that a reference to a missing or invisible row is a field error, not a foreign-key error.
7. As a developer, I want the target model's own scopes (soft delete, default scope, tenant) to apply to `unique` / `exists`, so that a soft-deleted row neither blocks nor satisfies a rule unless I say so.
8. As a developer, I want custom database rules (`database = path` on a field, `validate_with_database = path` on the record), so that checks like "the order belongs to this customer" fit the same report.
9. As a developer, I want database rules to run only on fields whose field rules passed, and all built-in database rules of an input in one statement, so that validation costs one round trip and never queries with malformed values.
10. As a developer, I want `validate` to take any session (database handle, pool, transaction), so that I can validate inside the write's transaction.
11. As a developer, I want a unique or foreign-key violation on `insert` / `update` to come back as `Error::Validation` with the field and code `unique` / `exists`, so that a race the rule lost still reads as a field error.
12. As a developer, I want transforms (`trim`, `lowercase`, `uppercase`, `normalize_email`, `custom = path`) applied before a field's rules and kept in the returned input, so that I validate and store the normalized value.
13. As a developer, I want `Option<T>` fields optional and plain fields required, and `required_if` / `required_unless` / `required_with` on `Option` fields (code `required`), so that presence can depend on other fields.
14. As a developer, I want `when = path` (`fn(&Self) -> bool`) on a field, so that its rules run only when a condition holds.
15. As a developer, I want enum inputs (one variant per shape) validated by their variant's rules, so that union bodies work.
16. As a developer, I want nested inputs and `Vec`s of them validated recursively, with errors at paths like `address.city` and `items.2.sku`, so that a client can point at the failing value.
17. As a developer, I want `message = "at least {min} characters"` on a rule, so that one message reads the way my product speaks.
18. As a developer, I want a message provider keyed by rule code (and optionally field), passed when errors are reported, so that I can translate every message in one place.
19. As a developer, I want an `ErrorReporter` whose default writes the generated APIs' `422 validation_failed` body, so that hand-written and generated endpoints answer alike.
20. As a developer, I want `Input::json_schema()` (JSON Schema 2020-12, feature `json`), so that my API documentation comes from the same declaration as the checks.
21. As a developer, I want the full rule catalogue (`url`, `mobile`, `postal_code`, `uuid`, `ip`, `regex`, `alpha`, `alpha_numeric`, `in`, `not_in`, `confirmed`, `same_as`, number, date and array bounds) on inputs and models, so that I don't reimplement what generated APIs already check.
22. As a developer, I want a database rule on a `#[derive(Model)]` struct to fail to compile with a message naming `Input`, so that I don't expect hidden queries in writes.
23. As a user of a generated API, I want the same answers as today after the engine adopts `#[derive(Input)]`, so that no client breaks.
24. As a maintainer, I want one implementation of each rule, used by the derive and by generated code, so that the two can't drift.

## Implementation Decisions

- **Name and shape:** `#[derive(Input)]`; `async fn validate<'c>(self, session: impl Into<Journey<'c>>) -> Result<Self, ValidationErrors>` (an `Error::Validation` on the session's own failures stays an `Error`; the signature returns `rok_data::Result<Self>` with `Error::Validation` for rule failures — settled in ticket 02).
- **Order:** transforms → field rules (every failure collected) → built-in database rules for fields that passed, in one `SELECT EXISTS(…) AS …` statement → custom database rules, one query each → record-level `validate_with` / `validate_with_database`.
- **Errors:** `FieldError { field: String (dotted path), code: &'static str, message: String, params }`; `ValidationErrors` shared by models and inputs. Codes are snake_case; the generated APIs' existing codes are the contract.
- **Constraint backstop:** the runtime maps SQLSTATE `23505` / `23503` on a model write to `Error::Validation` when the violated columns map to fields (PostgreSQL's detail names them); `is_unique_violation()` / `is_foreign_key_violation()` stay true.
- **Messages:** per-rule `message` beats a `Messages` provider beats the built-in English default; placeholders `{field}` and the rule's arguments.
- **Reporter:** `ErrorReporter` trait; the default renders `{"error": {"code": "validation_failed", "message": …, "details": {path: [{code, message}]}}}`, the generated APIs' body.
- **JSON Schema:** field rules map to keywords (`minLength`, `maximum`, `format`, `pattern`, `enum`, `required`); database rules and custom rules are left out.
- **Catalogue:** the engine's generated rule implementations move to `rok-data-core::validate` (split into a module per concern); generated code calls them.
- **Engine phase:** generated request DTOs derive `Input` instead of `validator::Validate`; their pre-check code is removed; `[api.rules]` stays the generator's input; the OpenAPI body schemas come from `json_schema`.

## Testing Decisions

- Facade suite (`crates/rok-data/tests/features`): SQL shape of the batched database-rule statement, and DB-backed behavior of every rule kind, skip-with-note without `DATABASE_URL`.
- Derive compile-fail tests (doctests) for misuse: database rule on a model, unknown rule, `except` naming a missing field.
- Engine: module fixtures and template gates refresh only through `ROK_DATA_SQLC_UPDATE_EXPECTED=1`; the templates' contract tests prove the wire answers unchanged.

## Out of Scope

- Database rules running implicitly inside writes.
- A compile-time "validated" wrapper type.
- Client-side validation code generation (only JSON Schema).

## Further Notes

- Breaking (unreleased section): `FieldError.field` becomes a `String` path; unique / foreign-key violations on writes become `Error::Validation`. Both go in `docs/migration-guide.md`.
- No framework names in the API, the docs or the codes.
