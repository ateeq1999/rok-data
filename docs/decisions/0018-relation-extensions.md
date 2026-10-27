# ADR 0018: relations describe their own batch

Date: 2026-10-27 · Status: accepted

## Context

0.3.0's relations (`BelongsTo`, `HasMany`, `HasOne`) all join one column
of the target to one column of the owner, and the loader assumed it:
`target_key IN (owner keys)`. 0.4.0 adds relations that don't: a country's
posts through its users (the target has no column holding the country), a
user's roles through a pivot table (one role belongs to many users). It
also adds filtering by a relation (`has`, `where_has`) and aggregates over
one (`with_count`).

## Decision

- The `Relation` trait gains hidden, overridable hooks — `batch_filter`,
  `batch_sql`, `batch_base`, `correlate`, `owner_of_target`, `targets` —
  with defaults that keep the direct relations as they were. The include
  loader, the `EXISTS` filters and the aggregates only use the hooks, so a
  new relation kind is one implementation, not a change to each consumer.
- **Through relations** (`HasManyThrough`, `HasOneThrough`) are declared
  (`#[rok(has_many_through(posts = Post::USER_ID, through =
  User::COUNTRY_ID))]`), not inferred from foreign-key chains: every chain
  would become a relation, named by guesswork.
- **Many-to-many** (`BelongsToMany`) is declared with its pivot table
  (`#[rok(belongs_to_many(roles = Role, pivot = "role_user"))]`, key
  columns defaulted from the model names) and *is* inferred by the
  generator for a join table pairing two primary keys, which is
  unambiguous. It writes the pivot (`attach`, `detach`, `sync`, `toggle`,
  …), each as one statement.
- **Aggregates** are includes parked on the loaded view (ADR 0017), not
  columns on the parent: `with_count`, `with_exists`, and handles
  (`User::POSTS.sum::<i64>(Post::VIEWS)`) whose value type picks the SQL
  cast, so a type that can't hold the result fails when the query runs.

## Consequences

- A relation's filters, loads and aggregates agree by construction: they
  are the same hooks.
- The engine writes `belongs_to_many` on both sides of a join table;
  generated models gain a line and an import.
- Polymorphic relations remain out of scope; when they come, they are one
  more `Relation` implementation.
