# ADR 0015: conventions are inferred, with explicit opt-outs

Date: 2026-10-22 · Status: accepted

## Context

A model must currently *declare* the conventions it wants:

```rust
#[derive(Model)]
#[rok(table = "users", timestamps, soft_delete)]
struct User { /* … */ }
```

and the generator does not emit those markers at all — a generated model has
no ORM-managed timestamps even when the table has `created_at` and
`updated_at`, and no soft delete even when the table has `deleted_at`. So the
ceremony is paid twice: by the developer who writes the model, and by the
generator, which cannot apply the convention it can see in the column names.

A column name can be the whole declaration: a `created_at`/`updated_at` pair *is* the signal,
a `deleted_at` column *is* the signal. The friction is that a developer who
knows the schema has to restate, in every model, what the table's column names
already say.

## Decision

The derive **infers** the conventions from the model's own fields, and the
generator applies the same rules:

| Signal | Inference | Opt-out |
| --- | --- | --- |
| fields named `created_at` and `updated_at` | ORM-managed timestamps | `#[rok(no_timestamps)]` |
| a field named `deleted_at` | soft deletes | `#[rok(no_soft_delete)]` |
| a field named `id` | primary key | `#[rok(primary_key = ...)]` (unchanged) |
| the struct name | table name (pluralized snake case) | `#[rok(table = "...")]` (unchanged) |

- The opt-out attributes are **new**. The existing `#[rok(timestamps)]` and
  `#[rok(soft_delete)]` markers keep working and mean the same thing — they
  become redundant, not wrong.
- The generator **stops writing** `#[rok(timestamps)]` and
  `#[rok(soft_delete)]`: it writes the opt-out only when the table has the
  column but the database owns it.
- **Managed timestamps, precisely.** A model with both timestamp columns gets
  `now()` written on insert and on every update — the runtime's existing
  `#[rok(timestamps)]` behaviour, unchanged. For *generated* models the
  generator turns this on only when the table's `created_at` and `updated_at`
  columns default to `now()`/`CURRENT_TIMESTAMP`; a table whose timestamp
  column has a different default keeps the database's value, and the
  generator writes the opt-out there. A column listed in the
  `managed_columns` convention is managed by the data layer regardless of its
  default.

## Considered Options

- **Keep the explicit declaration.** Rejected: it is ceremony per model for
  something the field names already say, and the generator cannot apply a
  convention it does not see declared.
- **Infer, with no way to turn it off.** Rejected: a table whose
  `deleted_at` column means something else (an audit trail, a soft-deleted
  account kept forever) would be silently swallowed by the data layer. The
  opt-out is the price of the inference.
- **Infer in the generator only.** Rejected: hand-written models and
  generated models would then behave differently for the same table, and the
  difference would only show up in production.

## Consequences

- The attribute grammar grows two opt-outs (`no_timestamps`,
  `no_soft_delete`); the two existing markers become redundant.
- Inference is visible in the generated code: the generated model either
  carries the convention (through the column names) or carries the opt-out
  that turns it off — a reader of a generated model can see which it is.
- A model whose fields do not follow the naming convention declares the
  convention explicitly, as today; nothing about the runtime's behavior
  changes for it.
- The generator's conventions section grows the same two opt-outs, so a
  project's config can say "this table's `deleted_at` is not a soft delete"
  for the tables where the column name lies.
