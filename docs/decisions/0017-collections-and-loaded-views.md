# ADR 0017: collections and loaded views carry what a query fetched

Date: 2026-10-27 · Status: accepted

## Context

0.4.0 (`docs/specs/0.4.0-everyday-verbs.md`) adds a collection of rows and
more things a query loads beside its rows: relation counts and sums,
nested relations. Two places could hold them: fields on the model (each row
"gains" `posts_count`), or the value the query returns.

A Rust struct has no field for an aggregate the query happened to ask for,
and a generated type per combination of loads is not a design.

## Decision

- **`ModelCollection<M>`** wraps the rows (`Deref<Target = Vec<M>>`) and
  offers the key verbs: `find`, `find_or_fail`, `contains_key`,
  `model_keys`, set operations by key, `to_query`, `fresh`, `load`,
  `load_count`. `Select::get` returns it; `all` keeps the `Vec`. It is
  built from a `Vec`, an array or an iterator, never through a derive
  method that would force `Clone`.
- **`Loaded<M>`** is the one place loaded extras live, keyed by the
  constant or handle that asked for them: relations (`try_get` / `get`),
  aggregates (`count`, `exists`, `aggregate(handle, row)`) and nested views
  (`nested(rel)`, itself a `Loaded` of the relation's records).
- Reading something the query didn't load is a caller bug: `try_get` says
  `None`, `get` fails with `Error::NotLoaded`, whose message names the fix.
  There is no process-wide "strict" flag: the runtime never loads
  implicitly, so there is nothing else to forbid.

## Consequences

- Rows stay the plain structs the schema describes; one query's extras
  never leak into another's type.
- Reading an extra takes the same constant that requested it, so a typo is
  a compile error and a forgotten `.with` a clear runtime one.
- A nested include clones its records into the nested view, so its target
  model must be `Clone` (generated models are).
