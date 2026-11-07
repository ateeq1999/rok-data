# 43: Request bodies that are easy to build, in every Client

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it) · engine ADR 0027

**What to build:** a field gets three states (left out, null, a value) only in a `PATCH` body, where the API tells them apart; elsewhere an optional nullable field is plain optional.

| Client | `PATCH` body field | Any other body field |
|---|---|---|
| Rust | `Option<Option<T>>` | `Option<T>` |
| Dart | `Nullable<T>?` | `T?` |
| TypeScript | `field?: T \| null` | `field?: T` |

- **`Default`:** every Rust body struct whose fields are all optional derives `Default`.
- **No compatibility switch:** the next release's migration guide shows the one-line call-site change.
- **ADR:** engine ADR 0027's Client sections are amended.

Karibu Pass met the Rust form in ticket 05 and the Dart form in ticket 22: callers wrote `guestName: name == null ? null : Nullable(name)` on a `POST`, and could send an explicit `null` by passing `Nullable(null)`.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] A `POST` body's optional nullable field is plain optional in each Client; a `PATCH` body's keeps three states (emission tests)
- [ ] An all-optional Rust body derives `Default`
- [ ] The API's client round-trip tests still pass; the monorepo Clients regenerate in rok-templates
