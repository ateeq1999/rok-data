# ADR 0001: rok-db is the runtime lineage

Date: 2026-10-17 (audit) · Status: accepted

## Context

Four repositories implement overlapping runtimes. Only one can be the
workspace's single runtime; the rest are documentation-only lineages.
The decision must be verifiable, not name-based.

## Decision

`rok-db` is the runtime lineage. The consolidated crates are:

| rok-db original   | rok-data crate           |
| ----------------- | ------------------------ |
| `rok-db-core`     | `rok-data-core`          |
| `rok-db-macros`   | `rok-data-macros`        |
| `rok-db` (facade) | `rok-data`               |
| `rok-db-codegen`  | `rok-data-codegen`       |
| `examples/sqlgen` | `examples/sqlgen` (kept) |

`rok-fluent` and `rok-orm` are **superseded**: their distinctive, still
valuable pieces were absorbed (`introspect` ← rok-fluent's
`SchemaInspector`; the single-file migration format ← rok-fluent's
convention, loader accepts it), and the API mapping is documented in the
migration guide. No compatibility shims are provided.

## Consequences

- sqlx 0.9 stays a single dependency in `rok-data-core`; PostgreSQL only

  (as rok-db was).

- The journal table is named `_rok_data_migrations`; fresh databases are

  created by the new name. Existing databases migrating from rok-db must
  rename the journal table (or keep it via the documented override).
- Every rok-db test suite was ported and passes, except DB-gated suites

  that legitimately print skip notes without `DATABASE_URL` (as the
  original did).
