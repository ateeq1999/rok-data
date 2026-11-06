# 66: Clients send `If-Match`

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** the engine's contract reader (`clients::openapi`) accepts header parameters, which it refuses today ("a parameter in `header` … is not supported"). Every generated `PATCH` on a versioned table carries `If-Match`, so `rok gen` can't emit Clients for such an App. The TypeScript, Dart and Rust Clients take a header parameter in the call's options, with the version for `If-Match` (`api.documents.update(id, body, { ifMatch: version })`), named after the header in each language's case. Cookie parameters and other unsupported constructs stay refused. Engine ADR 0027 is amended.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] api-app's served document (with its versioned `documents`) reads, and its Clients emit (test; remove ticket 61's header-stripping workaround in its guard test)
- [x] Each Client sends `If-Match` when given (emission tests, and the API's Rust-client round trip on a versioned update)
- [x] A cookie parameter is still refused

**Notes:** the API's Rust-client round trip on a versioned update is not
in rok-data: its API fixtures (`tests/api-app`, `tests/axum-app`) test
over HTTP without a generated Client, and the Rust-client round trips live
in rok-templates' monorepo. The emission tests cover each Client, and the
api-app document's Rust Client builds and passes rustfmt and clippy, its
Dart Client `dart analyze --fatal-infos`, its TypeScript Client `tsc
--strict` (checked by hand).
