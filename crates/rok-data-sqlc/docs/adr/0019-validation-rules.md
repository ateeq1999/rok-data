# Validation rules for generated APIs: pre-checks answer 422, string rules by VineJS name

Date: 2026-10-26 (imported) · Status: accepted upstream, imported; in force here as the absorption tickets port it

Formerly rok-sqlc ADR 0017 (rok-sqlc 0.5.0). Imported under workspace ADR 0016. The text is upstream's. The terms are this workspace's: the `rok sqlc` front door, `rok.toml`, and engine ADR numbers after the import.

> **In this workspace.** The checks are expressed on the generated model's shape (engine ADR 0018). The data-layer reads named below (`read::exists_by_id`, `listing::exists_owned`, `listing::<columns>_taken`) are upstream's per-module names. The port gives them the shape engine ADR 0018 sets for generated verbs, and keeps upstream's behaviour: what is checked, when, against which scope, and the answer. `[api.rules]` lives in `rok.toml` beside `[api]` (workspace ADR 0011).

A generated API (engine ADR 0016) answered a duplicate email or a dangling foreign key as `409 conflict` naming a database constraint. It had no way to check a format the schema can't express. Clients want answers they can show next to an input. So in rok-sqlc 0.5.0 a generated create or update checks, before it writes, what the database would reject, and answers it as a field's `422 validation_failed`. And `[api.rules]` adds string rules by VineJS's names. The decisions are Q52–Q54 of the 0.4 milestone notes (in the rok-sqlc repository).

**Pre-checks come from the schema, and run on the write's transaction.** `create_in` and `update_in` check, after the owner lock and before the write:

- every foreign key the body holds (present and not `null`) names a row the caller can see (`exists`);
- no other row holds the values of a unique key whose columns the body holds (`unique`).

Every failure goes into `FieldFailures` (`shared/validation.rs`), and they are answered at once, in the `details` shape of the body's rules: `{"email": [{"code": "unique", "message": "email is already taken"}]}`. The checks run after the body's own rules (the schema's `length`/`range`, `immutable`, and the string rules), so a body that fails those never queries the database. Each check is one `SELECT EXISTS (...)` in the data layer: `read::exists_by_id`, an owned table's `listing::exists_owned`, and `listing::<columns>_taken`.

**"Can see" is the read path's scope, so nothing leaks.** A referenced row counts as missing when:

- it doesn't exist;
- it is soft-deleted;
- it is in another tenant;
- for an owned table, it belongs to another caller.

So a `422 exists` says the same thing as the `404` a `GET` of that row would. It never confirms that another tenant's or caller's row exists.

To keep that true, two kinds of feature take an extra argument:

- A feature without an owner column still passes the caller to the check when its routes need one (`user` or `admin`).
- A feature without a tenant column takes the request's `Tenant` when it references a tenant-scoped table.

The service method of that create or update gains `user` or `tenant`, which is why its signature can differ from the feature's other methods. A unique key's tenant and owner columns come from the caller the same way. So `(tenant_id, name)` and `(owner_id, name)` are checked per tenant or owner, and reported on `name`.

How a unique key counts rows:

- A unique index on plain columns plus one `lower(<column>)` entry compares that column ignoring case.
- A partial index on `<soft-delete column> IS NULL` counts live rows only. A full constraint also counts soft-deleted rows, as the database does.
- An update never counts its own row.

**409 → 422 is a breaking change, and the 409 stays as the fallback.** A check and its write are two statements. A row deleted, or a duplicate inserted, between them still breaks the constraint, which answers `409 conflict` naming it. The checks don't lock what they read: a `FOR SHARE` on the referenced row, or a predicate lock for the unique key, would serialize unrelated writes. A client that gets a `409` retries and gets the `422`. Clients that branched on the `409` see a `422` instead. That is why this was rok-sqlc 0.5.0, listed under breaking changes to the generated API.

**What still answers 409, or isn't checked:**

- **A public route referencing an owned table.** There is no caller, so the check can only say that the row exists, live, in the tenant. It uses `exists_by_id`, and ownership is the application's to enforce: don't reference owned rows from public routes, or override `create_in`.
- **A reference to anything but the referenced table's whole single-column key**, of the key's own type. Also a reference to a tenant-scoped table whose tenant column has another type than the one the route already takes. The constraint decides.
- **A `PATCH` holding only some of a composite key's columns.** The others would have to be read from the row first, so the constraint decides.
- **A unique index with any other expression** (`upper()`, `coalesce()`, two `lower()` entries). The engine doesn't read it as a key at all.
- **Check constraints the rules don't cover**, as before. The database answers them without details.

**String rules take VineJS's names and meanings, or fail the config.**

- **Syntax:** `[api.rules.<table>.<column>]` lists rules (`email`, `minLength(3)`, `{ regex = "..." }`) and transforms (`trim`, `toLowerCase`, `toUpperCase`, `normalizeEmail`), in the order given.
- **Transforms:** they run while the body is read, so the stored value is the transformed one.
- **How a rule is implemented:** a rule maps to `validator`'s own (`email`, `ip`) where it means the same. Otherwise it maps to a function generated into `dto.rs`, calling helpers that `shared/validation.rs` holds once.
- **Refused rules:** a rule the engine can't give VineJS's meaning is refused with a clear message, not approximated. That covers `confirmed`, `sameAs` and `notSameAs` (cross-field, which belong in hand-written request types), and `activeUrl` (DNS during validation).
- **`url`:** a port of validator.js's `isURL` defaults, so `example.com` passes as it does in VineJS. `validator`'s own `url` would require a scheme.
- **Partial data:** where the engine carries only part of validator.js's data, the configuring guide documents the difference. `mobile` and `postalCode` know 16 locales and 7 countries, and `mobile` with no locale tries those 16, not every locale.

## Considered Options

- **Keep `409` and add field details to it.** Rejected: `409` means the request conflicts with the resource's state, and clients already treat `422` as "fix this field". One status for every field-level failure lets a form show them together.
- **Check inside the database (deferred constraints, `ON CONFLICT DO NOTHING ... RETURNING`) and map the violation to a field.** Rejected: one violation surfaces at a time, so a body with a duplicate email and a dangling foreign key needs two round trips. And the error carries only a constraint's name, not its columns.
- **Lock what the checks read, so the 409 fallback can't happen.** Rejected: the locks would serialize writes that don't conflict, for a race a retry already handles.
- **Count a row the caller can't see as present (only check existence).** Rejected: a `422` on another tenant's id but a `201` on a random one tells the caller that the other row exists.
- **Approximate rules the engine can't express exactly** (any locale for `mobile`, a scheme-requiring `url`). Rejected for `url`, which is now ported. Kept, and documented, for `mobile` without a locale, which Q54's own example (`phone = ["mobile"]`) uses.

## Consequences

A create or update with foreign keys or unique keys makes one extra query per checked field, on its transaction. Overriding `create_in` or `update_in` in `service.rs` drops the generated checks along with the rest of the default. The override can call the same data-layer reads and `FieldFailures`.
