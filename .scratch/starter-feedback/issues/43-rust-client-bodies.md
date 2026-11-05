# 43: Rust Client request bodies that are easy to build

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it) · engine ADR 0027

**What to build:** in the Rust Client, an optional nullable body field is `Option<Option<T>>` only in a `PATCH` body; elsewhere it is `Option<T>`. Every body struct whose fields are all optional derives `Default`. The engine ADR 0027 Rust section is amended.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] A `POST` body's optional nullable field is `Option<T>`; a `PATCH` body's is `Option<Option<T>>`
- [ ] An all-optional body derives `Default` (emission tests)
- [ ] The API's client round-trip tests still pass
