# ADR 0020: chain sugar stays typed on the column; row verbs reuse what exists

Date: 2026-10-27 · Status: accepted

## Context

The 0.4.0 spec listed query sugar in a stringly shape —
`where_any(columns, op, value)`, `where_date(col, date)`,
`where_column(col1, op, col2)` — and row verbs with near-duplicates of
existing ones: `update_or_create` beside `upsert_on`, `fresh` beside
`reload`, `replicate`, `insert_get_id`.

## Decision

- **Conditions live on the column**, typed, and compose with `filter`,
  `and`, `or` like every other condition: `PUBLISHED_AT.year().eq(2026)`,
  `.date()`, `.json_array_len()`, `.json_has_path([…])`,
  `.gt_column(OTHER)`, `.is_not_distinct_from(v)`. `Select` gains sugar
  only where it reads better than a condition: `filter_any([cols], |c|
  …)` / `filter_all` / `filter_none`, raw escapes (`filter_raw`,
  `group_by_raw`, …), `pipe` / `tap` / `debug`, `union` / `union_all`
  (a `Union` builder). New chain names follow the canonical verb,
  `filter`.
- **One name per operation.** `update_or_create` is `upsert_on`, `fresh`
  is `reload`, `replicate` is `insert` (inserts skip generated keys and
  timestamps) or a struct update, and `insert_get_id` is `exec(..)?.id` (a
  model's key is always one of its fields). The new verbs are the ones
  with no equivalent: `first_or_insert` (one atomic statement),
  `increment` / `decrement` / `touch`, and `select.insert_into::<T>(…)`
  for `INSERT … SELECT`.
- `debug_raw` (bound values inlined into the SQL) is not offered: the
  string would look runnable while losing the parameters' types.
- No framework's names are used for the API (the workspace convention);
  the docs say what a call does.

## Consequences

- Every new condition works wherever a condition does — in `filter`,
  `having`, `where_has`, an include's shaping — without a `Select`
  method per combination.
- The surface stays small: a reader learns one verb per operation.
