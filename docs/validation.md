---
title: Validating input
description: "#[derive(Input)]: transforms, field rules, database rules, context, conditional rules, nesting, messages, the 422 body, JSON Schema, and what generated APIs do with it."
updated: 2026-11-10
kind: guide
source: crates/rok-data/tests/docs/validation.rs (compiled samples)
---

# Validating input

## Agent brief

- **What this page does:** checking untrusted data (a request body, a
  form, a job's payload) before it becomes rows, with
  `#[derive(Input)]` (workspace ADR 0021; engine ADR 0025 for generated
  APIs).
- **The call:** `input.validate(&db).await?` returns the transformed
  input or `Error::Validation` with every failure;
  `validate_with(&db, &context)` when the input declares
  `#[rok(context = …)]`; `validate_fields()` runs no database rule and
  needs no session.
- **The order:** transforms, then field rules (every failure collected),
  then the built-in database rules (`unique`, `exists`) of the fields
  that passed, in one statement, then custom database rules, one query
  each.
- **Constraints:** database rules belong to inputs, never to a
  `#[derive(Model)]` (a compile error says so); the table's constraint
  stays the guarantee, and a unique or foreign-key violation on a write
  comes back as the same field error.
- **Samples are compiled** (`crates/rok-data/tests/docs/validation.rs`),
  marked `<!-- source: … -->`: edit that file, not this page.

An [**Input**](../GLOSSARY.md) is data on its way to becoming rows. Its
[**Field rules**](../GLOSSARY.md) check a value's shape; its [**Database rules**](../GLOSSARY.md) check what
depends on rows (a taken email, a missing organization). Every sample
below is compiled.

## An input

<!-- source: crates/rok-data/tests/docs/validation.rs -->
```rust
// an input: transforms, then field rules
#[derive(Debug, Clone, Input)]
struct SignUp {
    #[rok(transform(trim, normalize_email), validate(email))]
    email: String,
    #[rok(transform(trim), validate(length(min = 2, max = 40)))]
    name: String,
    // `Option` is optional: its rules run only on `Some`
    #[rok(validate(url))]
    website: Option<String>,
}
```

<!-- source: crates/rok-data/tests/docs/validation.rs -->
```rust
// the transformed input, or `Error::Validation` with every failure
let input = body.validate(&db).await?;

// no database rules to run, or a test: no session needed
let input = input.validate_fields()?;
```

A failure carries a dotted path, a code and a message:

<!-- source: crates/rok-data/tests/docs/validation.rs -->
```rust
if let rok_data::Error::Validation(errors) = error {
    for failure in errors.errors() {
        // the dotted path (`lines.2.sku`), the rule's code, its message
        println!("{} / {} / {}", failure.field, failure.code, failure.message);
    }
}
```

## Field rules and transforms

Field rules are spelled as on a model, `#[rok(validate(…))]`, so a rule
moves between a model and an input unchanged. Transforms run first, in
order, and stay in the returned input.

| Kind | Spelling |
| ---- | -------- |
| transforms | `trim`, `lowercase`, `uppercase`, `normalize_email`, `custom = path` (`fn(T) -> T`) |
| text | `length(min, max)`, `min_length`, `max_length`, `fixed_length`, `non_empty`, `email`, `url`, `regex = "…"` (feature `regex`), `alpha`, `alpha_numeric` (each with `dashes`, `underscores`, `spaces`), `ascii`, `starts_with = "…"`, `ends_with = "…"` |
| identifiers | `uuid(versions = [4])`, `ulid`, `ip(version = 4)`, `jwt`, `hex_code`, `iban`, `credit_card(providers = ["visa"])` |
| places and phones | `mobile(locales = ["ar-SA"], strict)`, `postal_code(countries = ["US"])`, `coordinates` (both feature `regex`) |
| sets | `one_of(values = […])`, `not_one_of(values = […])` |
| numbers | `range(min, max)`, with `exclusive_min` / `exclusive_max`; a `NUMERIC` takes it directly |
| presence | a plain field is required, an `Option` field optional; `required_if(field = f, equals = "…")`, `required_unless(…)`, `required_with(f)` on an `Option` (code `required`) |
| your own | `custom(function = path)` (`fn(&T) -> Result<(), String>`), with `code = "…"` for a code of its own |

## Database rules

`unique(Model::COLUMN)` fails when a row already holds the value;
`exists(Model::COLUMN)` fails when none does. Both take `scope = path`
(`fn(&Self) -> Expr<Model>`), and the target model's own scopes (soft
delete, a default scope) apply. `unique` also takes `case_insensitive`
and `except = field` for the row being updated. All of an input's
built-in database rules run in one `SELECT`, and only for fields whose
field rules passed. A custom one is `database = path` on a field, or
`validate_with_database = path` on the input.

<!-- source: crates/rok-data/tests/docs/validation.rs -->
```rust
// database rules: checked in one statement, for fields whose field rules passed
#[derive(Debug, Clone, Input)]
#[rok(validate_with_database = not_reserved)]
struct NewUser {
    #[rok(
        transform(trim, normalize_email),
        validate(email, unique(User::EMAIL, case_insensitive))
    )]
    email: String,
    #[rok(validate(exists(Org::ID, scope = active_orgs)))]
    org_id: i64,
    #[rok(validate(database = handle_free))]
    handle: Option<String>,
}

fn active_orgs(_input: &NewUser) -> Expr<Org> {
    Org::ACTIVE.eq(true)
}

async fn handle_free(handle: &Option<String>, session: Journey<'_>) -> Result<(), String> {
    match handle.as_deref() {
        Some("admin") => Err("that handle is reserved".into()),
        _ => Ok(()),
    }
}

async fn not_reserved(input: &NewUser, session: Journey<'_>) -> Result<(), ValidationErrors> {
    let mut errors = ValidationErrors::new();
    if input.email.starts_with("root@") {
        errors.add("email", "reserved", "that email is reserved");
    }
    errors.into_result()
}
```

<!-- source: crates/rok-data/tests/docs/validation.rs -->
```rust
// updating a row: it doesn't collide with itself
#[derive(Debug, Clone, Input)]
struct EditUser {
    id: i64,
    #[rok(validate(unique(User::EMAIL, except = id)))]
    email: String,
}
```

`validate` takes any session, so it can run inside the write's
transaction. That narrows the window in which another write breaks a
rule; it doesn't close it. The table's constraint is the guarantee: a
unique or foreign-key violation on a model's write comes back as
`Error::Validation` with the column's field and code `unique` /
`exists`, the error the rule would have given.

<!-- source: crates/rok-data/tests/docs/validation.rs -->
```rust
let mut tx = db.begin().await?;
let input = body.validate(&mut tx).await?;
// … insert the rows on `tx` …
tx.commit().await?;
```

## Who is asking: a context

When a rule depends on the caller (a row *this caller* can see), the
input declares a context. Its scopes and custom database rules receive
it, and `validate_with` passes it.

<!-- source: crates/rok-data/tests/docs/validation.rs -->
```rust
// who is asking: what a context-aware rule reads
struct Caller {
    org_id: i64,
}

#[derive(Debug, Clone, Input)]
#[rok(context = Caller)]
struct MoveUser {
    #[rok(validate(exists(User::ID, scope = in_callers_org)))]
    user_id: i64,
    #[rok(validate(database = not_own_org))]
    to_org: i64,
}

fn in_callers_org(_input: &MoveUser, caller: &Caller) -> Expr<User> {
    User::ORG_ID.eq(caller.org_id)
}

async fn not_own_org(to_org: &i64, session: Journey<'_>, caller: &Caller) -> Result<(), String> {
    if *to_org == caller.org_id {
        Err("the user is already in that org".into())
    } else {
        Ok(())
    }
}
```

<!-- source: crates/rok-data/tests/docs/validation.rs -->
```rust
let input = body.validate_with(&db, &caller).await?;
```

## Conditional rules, nesting and record rules

`when = path` (`fn(&Self) -> bool`) runs a field's rules only when it
holds. `#[rok(nested)]` validates an input inside an input (or an
`Option` or a `Vec` of them), with errors at `address.city` and
`lines.2.sku`; a nested input shares its parent's context.
`validate_with = path` on the input runs after the field rules, on the
transformed input.

<!-- source: crates/rok-data/tests/docs/validation.rs -->
```rust
// conditional rules, nesting and a record rule
#[derive(Debug, Clone, Input)]
#[rok(validate_with = names_differ)]
struct Order {
    kind: String,
    #[rok(validate(required_if(field = kind, equals = "company")))]
    company: Option<String>,
    #[rok(validate(required_unless(field = kind, equals = "pickup")))]
    phone: Option<String>,
    weekend: bool,
    #[rok(when = on_weekends, validate(length(min = 5)))]
    coupon: String,
    #[rok(nested)]
    address: Address,
    #[rok(nested)]
    lines: Vec<Line>,
}
```

An enum input, one variant per shape, runs its variant's rules:

<!-- source: crates/rok-data/tests/docs/validation.rs -->
```rust
// one body, two shapes: the variant's rules run
#[derive(Debug, Clone, Input)]
enum Customer {
    Person(Person),
    Company(Company),
}
```

## Messages

A rule's own `message` beats a provider's, which beats the built-in
English one. Placeholders are `{field}` and the rule's arguments
(`{min}`, `{max}`, …).

<!-- source: crates/rok-data/tests/docs/validation.rs -->
```rust
// a rule's own message, with the rule's arguments as placeholders
#[derive(Debug, Clone, Input)]
struct Profile {
    #[rok(validate(length(min = 3, message = "{field} needs {min}+ characters")))]
    handle: String,
    #[rok(validate(custom(function = not_admin, code = "reserved")))]
    nickname: String,
}
```

A provider keys every message by the rule's code (and the field, when
it matters), so a product translates them in one place:

<!-- source: crates/rok-data/tests/docs/validation.rs -->
```rust
// every message in one place, keyed by the rule's code
struct Swahili;

impl rok_data::validate::Messages for Swahili {
    fn template(&self, code: &str, field: &str) -> Option<String> {
        match code {
            "length" => Some("{field} ni fupi mno (angalau {min})".to_owned()),
            "email" => Some("barua pepe si sahihi".to_owned()),
            _ => None,
        }
    }
}

fn translated(errors: ValidationErrors) -> ValidationErrors {
    errors.localized(&Swahili)
}
```

## The 422 body and JSON Schema

`DetailsReporter` (feature `json`) writes the body the generated APIs
answer with, so hand-written and generated endpoints answer alike.
Record-level errors sit under `""`.

<!-- source: crates/rok-data/tests/docs/validation.rs -->
```rust
use rok_data::validate::{DetailsReporter, ErrorReporter};

// {"error": {"code": "validation_failed", "message": …,
//            "details": {"email": [{"code": "email", "message": …, "params": {}}]}}}
DetailsReporter.report(errors)
```

`Input::json_schema()` (feature `json`) describes the same input for
documentation and clients. Database rules and custom rules are left out:
a schema can't say them.

<!-- source: crates/rok-data/tests/docs/validation.rs -->
```rust
// JSON Schema 2020-12: types, `required`, and the rules a schema can say
SignUp::json_schema()
```

## In a generated API

Generated create inputs, update patches and API bodies derive
`rok_data::Input` (engine ADR 0025). The schema's length and range
checks and the `[api.rules]` table become the `#[rok(…)]` attributes
above, so a generated body reads like a hand-written one.

- **The extractor:** `ValidatedJson<T>` takes any context-free `Input`
  and runs `validate_fields()`: transforms and field rules, and a 422 on
  failure.
- **Database rules** need a session, so the generated service runs its
  `unique` and `exists` pre-checks (engine ADR 0019), reported through
  the same `ValidationErrors`.
- **A hand-written body** that goes through `ValidatedJson` derives
  `Input` too. Moving one from `validator::Validate`: see
  [`migration-guide.md`](migration-guide.md), "Generated inputs derive
  `rok_data::Input`".
