# 66: Clients send `If-Match`

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** the engine's contract reader (`clients::openapi`) accepts header parameters, which it refuses today ("a parameter in `header` … is not supported"). Every generated `PATCH` on a versioned table carries `If-Match`, so `rok gen` can't emit Clients for such an App. The TypeScript, Dart and Rust Clients take a header parameter in the call's options, with the version for `If-Match` (`api.documents.update(id, body, { ifMatch: version })`), named after the header in each language's case. Cookie parameters and other unsupported constructs stay refused. Engine ADR 0027 is amended.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] api-app's served document (with its versioned `documents`) reads, and its Clients emit (test; remove ticket 61's header-stripping workaround in its guard test)
- [ ] Each Client sends `If-Match` when given (emission tests, and the API's Rust-client round trip on a versioned update)
- [ ] A cookie parameter is still refused
