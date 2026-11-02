# 02: the data layer on the generated Rust client

**Blocked by:** 01, and ADR 0027's ticket 07 (`clients/rust`)
**Status:** done

Spec: What ships (the data layer).

The repositories call `clients/rust`; the starter's hand-written HTTP calls
go. The client's validation errors still reach form fields. `clients/rust` is
a member of the monorepo's root workspace.

Done when no hand-written HTTP call is left in `apps/desktop`, and a change to
the API's contract that `rok gen` regenerates into the client breaks or flows
through the desktop App's build.

- [x] The repositories call `clients/rust` (`api-client`, a path dependency)
- [x] The starter's hand-written HTTP providers are gone
- [x] The client's validation errors reach form fields
- [x] A contract change regenerated into the client breaks or flows through the App's build

## Comments

**The provider.** `src/data/providers/api.rs` holds `Api`: the generated
`api_client::Client` wired to rok-ui's `Session` (signing in sets the token on
both; a 401 to a call that sent the session's token clears the client's token
and expires the session, as the starter's `HttpClient` did), and `app_error`,
which turns the client's `ApiError` into rok-ui's (`details` kept by field), so
blocs, `is_cancelled` and `form::to_server_errors` keep working unchanged.

**Mapping.** `AuthApi.sign_in` (`POST /sessions`) is now `auth().login` then
`auth().me()`; a two-factor account's pending answer is an error
(`totp_required`), as the App asks no code. `AuthApi.sign_out`
(`DELETE /sessions/current`) has no API counterpart: signing out forgets the
token. `AuthApi.me` (`GET /me`) is `auth().me()`. `NotesApi` has none (the API
serves no notes): the notes provider, repository, feature, route and tests are
gone, and `UsersRepository` (`users().list` a page at a time, `users().get_one`)
takes its place in the data layer for ticket 04's screens.

**Forced changes outside the data layer.** `UserId` is the API's `String`
(UUID), and `User.name` is the display name (empty without one). The auth
form's `emailAddress` mapping went: the API names its fields `email` and
`password`, the form's own. The shell lost its Notes link.

**reqwest.** No version move: rok-ui 0.9.1's `rok-ui-http` uses reqwest
0.12.28, the client's, so the App links one reqwest (`cargo tree -p desktop`);
the lock's 0.13.5 is `apps/api`'s OpenTelemetry exporter's. The root
`Cargo.lock` changed only in `desktop`'s entry.

**Gates.** `tests/data/` (8 tests) drives the repositories against rok-ui's
mock API on localhost; `rok-templates`' catalog test fails if the desktop
manifest stops depending on `clients/rust` by path or a source names
`HttpClient`, `http::Options` or `reqwest`.
