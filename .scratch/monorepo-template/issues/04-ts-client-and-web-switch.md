# 04: Generated TS Client; the web App talks through it

**What to build:** The Clients become real, per ADR 0027 — rok emits them itself from the api's `openapi.json`, not from the engine's model (hand-written auth/2FA routes exist only in the spec). Ticket for the TS one: the api App gains a genuine `openapi.json` artifact (the served document written out; refreshed only through the sanctioned expected-snapshot mechanism), rok reads the OpenAPI 3.1 subset utoipa emits — no more — and emits `clients/ts`: typed calls for every path, plus TanStack Query `queryOptions`/`mutationOptions` factories, keeping the web starter's `ApiError`/`FieldIssue` error shape. The web App's hand-rolled `http.ts` and per-feature api modules are replaced by the generated client: its screens now call through it, and the gate still passes. Init emits `clients/ts` into a fresh scaffold; `rok gen` regenerates it (and re-exports the artifact first, so contract changes flow through); generated-marker rules apply.

**Blocked by:** 03 (web App imported — its data layer is what this ticket replaces).

**Status:** done

- [x] The api App's served document is exported to a real `openapi.json`; a template-level check keeps it honest with permitted regeneration (never hand-edited), same practice as the repo's other expected snapshots
- [x] rok parses the utoipa subset (paths, methods, schemas, request bodies, statuses, auth marker) and rejects gracefully what it doesn't support, naming the unsupported construct
- [x] The TS client: typed calls with request/response types per operation,auth/session concerns preserved (bearer/cookie handling matching the web starter's behavior today), `ApiError` with `FieldIssue` for validation responses, and `queryOptions`/`mutationOptions` factories usable by TanStack Query callers
- [x] The web App's data layer is generated code: the starter's own HTTP module and feature api files are gone; screens call through the client; a contract change in the api spec changes the client output
- [x] Init emits `clients/ts` (as generated files, with whatever marker/marker-policy implications regeneration expects); `rok gen` re-exports + re-emits it; drift behavior for the client is defined and tested
- [x] The web typecheck/tests gate passes with the client in the loop, and a spec-driven regeneration round-trips through the gate; format/clippy/engine/rok-cli/rok-templates suites green

## Comments

**Where things live.** The reader (`clients::openapi`) and the TS emitter (`clients::ts`) are in the engine, beside `clients::Project` (the `[contract]`/`[clients]` sections of the monorepo's `rok.toml`) and `clients::generate` (missing files for init, up to date for `rok gen`, drift for `rok gen check`): the engine owns generated output and its marker rules, and the `init` that emits `clients/ts`. `rok gen` (rok-cli, `commands/gen/clients.rs`) re-exports the contract after the Apps and delegates. Engine ADR 0027 records it.

**The artifact.** `apps/api/examples/openapi.rs` writes `openapi::document()` to a path; `[contract].export` runs it (`cargo run -p axum-api --example openapi -- <document>`). The api's `tests/openapi_artifact.rs` fails in a user's project when the file is not what the API serves. The template's copy (and `clients/ts`) refresh only through the `ROK_DATA_SQLC_UPDATE_EXPECTED=1` run of `fixture_compile/monorepo_template.rs`.

**Drift behaviour.** `rok gen`: changed client files rewritten, a file whose marker is gone refused (the run fails) unless `--force`, a marked file no longer emitted removed as stale; `--dry-run` neither exports nor writes. `rok gen check`: builds and writes nothing, emits in memory from the checked-in document, fails on any missing/changed/stale file. Whether the document matches what the API serves is the api App's own test. Covered by `crates/rok-cli/tests/gen_clients.rs`.

**Session handling.** The starter keeps a bearer token in `localStorage` and sends no cookies; the client does the same through `configure({ baseUrl, token, onUnauthorized })`, wired to `lib/session.ts` in `apps/web/src/lib/client.ts`. The token goes only to operations carrying the bearer marker, and only their 401 reports the session over — the starter's `skipExpire` on public calls now follows from the spec; the two-factor setup/enable/disable calls (secured, where a 401 means a wrong code or password) pass `keepSession: true`.

**Hidden features** (their endpoints are not in `apps/api`'s served spec; entry points removed, no API routes invented):

- Passkeys: the login button and the security settings card (`features/passkeys/{api,queries}.ts`, its components and `schemas.ts` removed; `webauthn.ts` and its test kept).
- Sign-in by QR code: the login dialog and the `/qr-login` approval route (`features/qr-login` removed whole, see below).
- Sign-in with Google/GitHub (OAuth): the social buttons on login and register, the connected accounts card in account settings, and the `/oauth/callback` route (`features/oauth/{api,queries,schemas}.ts` and its components removed; `messages.ts` and its test kept).
- Creating a user with a password (`POST /api/v1/users` is not served): the admin "Create user" dialog and its dashboard/users-page buttons; inviting a user stays.

**Tests adapted** (127 → 120 tests, 19 → 18 files):

- `lib/__tests__/http.test.ts` and `http-blob.test.ts` tested the removed `lib/http.ts`; they became `client.test.ts` and `client-blob.test.ts`, the same behaviours through the generated client as the App configures it (bearer and JSON headers, raw blob bodies, extra headers, `apiUrl`, 204, the error contract, network errors, fallback messages for 413/415, a bytes answer, every 401 rule with `keepSession` in place of `skipExpire`), plus one new case: a public operation gets no token and its 401 keeps the session.
- `features/auth/login-flow.test.ts`: its `loginResponseSchema` block (3 tests) tested the zod parse of the login answer, which the generated `LoginResponse` type replaces; `loginStepFor` is now fed typed values.
- `features/qr-login/use-qr-login.test.ts` (5 tests) is removed with the hook it tested, which polled the QR endpoints the API does not serve.

**Judgement calls.** The generated client types responses but does not validate them at run time (the zod response schemas went with the api modules); the API's contract test and `rok gen check` keep the two sides together. `GET /api/v1/media/{id}/content` now documents its `200` as `application/octet-stream` in all three API templates, so the client reads it as a `Blob`. The client sits outside `apps/web`, so `apps/web/tsconfig.json` maps `@client` to it and `@tanstack/react-query` to the App's copy, and the Vite configs dedupe TanStack Query (dev, build and vitest checked). Init's ticket order: the init/gen emission (checkbox 5) was committed before the web switch (checkbox 4), so every commit passes the web gate.
