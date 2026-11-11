# 86: `COALESCE($n, column)` infers the column's type

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** a placeholder that is an argument of `COALESCE` beside a column reference (`COALESCE($2, locale)` or `COALESCE(locale, $2)`) infers `Option<the column's value type>`, for every column type: text, a typed id, a SQL enum, a CHECK-list enum. Today it fails with "cannot infer the type of `$1` from its context" for all of them. A `-- param:` override still wins.

Karibu Pass's `UpdateProfile` (`SET locale = COALESCE($2, locale)`) hit the failure and was written `-- param: … Option<String>` with `.as_str()` at the call. An override can already say `Option<types::UserLocale>`, but the inference should give that with no annotation (ticket 26).

**Blocked by:** None (can start immediately)

**Status:** done

- [x] `SET locale = COALESCE($2, locale)` over a CHECK-list column infers `Option<types::UserLocale>` (analyzer test)
- [x] The same over a text, a typed-id and a SQL-enum column, in both argument orders
- [x] A `COALESCE` that names no column still asks for `-- param:`
