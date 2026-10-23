# Spec — Absorb rok-sqlc 0.5–0.8 into the engine, then freeze rok-sqlc

Status: ready-for-agent · Tickets: `.scratch/absorb-rok-sqlc-0.8/issues` (ports blocked by `.scratch/dx-first`) · Release: lands in the workspace release after it is complete (minor; the templates carry breaking changes of their own, listed in the CHANGELOG) · Touches: `rok-data-sqlc` (engine), `rok-cli` (test suites, `rok sqlc` forwarding unchanged), docs · Implements: a new workspace ADR (absorption and freeze) and engine ADRs 0019–0022 (imported).

---

## Problem Statement

The `rok sqlc` engine inside this workspace was imported from the
standalone `rok-sqlc` repository at **v0.4.0**, as a plain copy with no
shared history. Since then both sides kept moving:

- **Upstream** shipped four releases (0.5 validation rules, 0.6 auth,
  0.7 actix hardening and axum, 0.8 two-factor sign-in, API keys, uploads
  and the storage port): 69 commits, roughly +74k / −7k lines over 530
  files.
- **This workspace** reshaped the engine on its own: `rok.toml` under
  `[generate]` (workspace ADR 0011), the model-centric generated shape
  (engine ADR 0018), inferred conventions, named scopes, its own axum
  support, snapshot gates. 153 of the 193 engine files the two copies
  share now differ from upstream v0.4.0.

A user of `rok sqlc init`/`generate` therefore gets a template without
two-factor sign-in, without the 0.7 security fixes (passwords hashed off
the async runtime, the `cross-feature-query` architecture rule), without
`415 unsupported_media_type`, and without the 0.5 validation rules — while
the same author's standalone tool has all of it. Two copies of one engine
also means every fix lands twice, or only once and the other copy rots.

The engine ADR series already collides: upstream 0017–0020 and this
workspace's engine 0017–0018 share numbers with different decisions.

## Solution

Bring every user-visible capability of rok-sqlc 0.5.0 through 0.8.0 into
`rok-data-sqlc`, **release by release**, translated into this workspace's
shape (the `rok sqlc` front door, `rok.toml`, the model-centric generated
code, the workspace's test layout). When the last release is absorbed,
rok-sqlc is **frozen** at 0.8.0: its README points here and it takes no
new features. From then on there is one engine.

The absorption is a port, not a merge: upstream commits are replayed as
behavior onto this workspace's code, and where the two sides decided the
same question differently the workspace's decision stands unless this
spec says otherwise.

## User Stories

### Scaffolding and templates

1. As a developer starting a project, I want `rok sqlc init` to scaffold the same auth stack rok-sqlc 0.8 does, so that I don't have to choose between the workspace tool and the standalone one.
2. As a developer, I want `rok sqlc init --framework actix|axum` to produce templates at feature parity with each other, so that my framework choice doesn't cost me features.
3. As a developer, I want the template's `AuthUser` to check the database on every request, so that a revoked or deleted account loses access immediately (0.6).
4. As a developer, I want the template's auth migration (`0002_auth`) and two-factor migration shipped as versioned migrations in this workspace's migration format, so that `rok migrate` applies them like any other.
5. As an operator, I want `JWT_SECRET` shorter than 32 bytes to stop the server at startup, so that a weak secret never reaches production (0.6).
6. As an operator, I want the actix template to hash passwords on a blocking thread, so that a burst of sign-ins cannot stall the async runtime (0.7).
7. As an API client, I want the actix `/api/v1/auth` scope to answer an unknown path or method with the wire-standard error, so that both frameworks behave identically (0.7).
8. As a user of the API, I want two-factor sign-in (setup, enable, disable, a challenge step at login, recovery codes), so that my account is protected beyond a password (0.8).
9. As an operator, I want two-factor secrets sealed with `TOTP_ENCRYPTION_KEY` and bound to their account, and recovery codes stored as keyed HMACs, so that a database leak does not leak usable second factors (0.8).
10. As a user, I want disabling two-factor to sign every session out, so that an attacker holding a session cannot quietly downgrade my account (0.8).
11. As a user, I want a password reset to revoke my API keys and pending two-factor sign-ins, so that a reset is a real recovery (0.8).
12. As a developer integrating a service, I want API keys (list, create, revoke) accepted by `AuthUser` as a bearer token, so that machines can call the API without a user session (0.8).
13. As an operator, I want an encoded path never to let an API key reach `/auth` or `/api-keys`, so that keys cannot mint more keys or change credentials (0.8).
14. As a user, I want to upload media (a raw body with a size cap) and get `413` for too large and `415 unsupported_media_type` for a disallowed type, so that failures are explicit (0.8).
15. As a user, I want to set and remove a profile photo, served publicly, so that the template covers the common avatar case (0.8).
16. As an operator, I want uploads stored through a `Storage` port opened from `STORAGE_URL` (local disk, S3-compatible), so that I choose the backend by configuration, not by code (0.8).
17. As a developer on actix, I want an upload's body read only after `AuthUser` succeeds, so that unauthenticated clients cannot make the server buffer bodies (0.8).
18. As a developer, I want two-factor to be its own template feature, so that I can leave it out of a project that does not need it (0.8).

### Generated API and validation

19. As a developer, I want `[api.rules]` in `rok.toml [generate]` to attach string rules and transforms by VineJS name per table and column, so that validation is declared once, next to the schema (0.5).
20. As an API client, I want a create or update whose foreign key names a missing row to answer `422` with the field named, so that I can show the error on the right input (0.5 `exists` pre-check).
21. As an API client, I want a create or update that would violate a unique index to answer `422` with the field named, instead of a bare `409`, so that duplicate-email style errors are field errors (0.5 `unique` pre-check).
22. As a developer, I want unique indexes on `lower(column)` treated as case-insensitive by the pre-check, so that `A@x.com` and `a@x.com` collide the way the database says they do (0.5).
23. As a developer, I want `regex` rules checked when `rok sqlc generate` runs, so that a bad pattern fails generation, not a request (0.5).
24. As a developer, I want the generated `shared/validation` to collect every field failure of a write (`FieldFailures`), so that a client sees all errors at once (0.5).

### Schema reading

25. As a developer whose schema comes from `pg_dump`, I want `ALTER TABLE … DROP CONSTRAINT [IF EXISTS]` followed, so that a dropped constraint stops shaping the generated code (0.8).
26. As a developer, I want dropping an unknown constraint to warn rather than fail the table, so that a dump with history I don't model still generates (0.8).
27. As a developer, I want `ADD CONSTRAINT … CHECK (col = ANY (ARRAY[...]))` read as a value list, so that pg_dump's spelling of an enum-like check produces the same validation as the hand-written one (0.8).
28. As a developer, I want `RENAME CONSTRAINT` to check both names, and `IS NULL` matched in any case, so that schema reading is not fooled by spelling (0.8).

### Architecture rules

29. As a team lead, I want `rok sqlc check-arch` to flag a feature that queries another feature's tables (`cross-feature-query`), so that feature boundaries hold (0.6).
30. As a team lead, I want `check-arch` to flag a feature that imports the other framework's crate, so that an actix project never grows an axum dependency by accident (0.7).

### One engine, one history

31. As a maintainer, I want rok-sqlc's README to say it is frozen at 0.8.0 and point to `rok sqlc`, so that new users land on the maintained engine.
32. As a maintainer, I want each absorbed upstream release recorded in the engine CHANGELOG with its upstream version, so that "is fix X in the workspace?" has a written answer.
33. As a maintainer, I want upstream engine ADRs 0017–0020 present in this workspace as engine ADRs 0019–0022, each naming its upstream number, so that every decision the code relies on is readable here without collisions.
34. As a maintainer, I want a workspace ADR recording the absorption, the freeze and the renumbering rule for imported ADRs, so that the next absorption (if any) follows the same rule.
35. As a maintainer, I want `docs/migration-audit.md` updated to say rok-sqlc is absorbed through 0.8.0, so that the audit stays true.
36. As a contributor, I want the scaffolded `AGENTS.md` and generated `features/README.md` to use this workspace's wording (`rok sqlc`, `rok.toml`), so that generated projects don't send me to a tool I don't have.
37. As a user reading `docs/cli-reference.md`, I want every grammar change (new `init` flags, new `[generate]` keys) documented in the same change that adds it, so that the reference stays a contract.
38. As a user upgrading, I want the CHANGELOG's breaking-changes section to list each new required environment variable, migration and wire answer, so that I can upgrade a scaffolded project deliberately.

## Implementation Decisions

- **Port by release, one ticket per release, in order 0.5 → 0.6 → 0.7 → 0.8.** Each ticket ends green on every workspace gate. Later releases build on earlier ones upstream; porting out of order would re-derive their dependencies.
- **Port behavior, not files.** For each upstream commit in the release range, reproduce the user-visible behavior on this workspace's code. Copying an upstream file over a workspace file is allowed only where the workspace copy is still byte-identical to upstream v0.4.0.
- **Translations applied on every port:**
  - tool name `rok-sqlc` → front door `rok sqlc` and crate `rok-data-sqlc`; marker headers accept both spellings (workspace ADR 0005);
  - `rok-sqlc.toml` keys → `rok.toml` under `[generate]` (workspace ADR 0011), with `[api.rules]` and `[api] framework` nested the same way the workspace nests `[api]` today;
  - generated code follows the workspace's model-centric shape (engine ADR 0018) where upstream emits per-module verbs;
  - template migrations use the workspace's versioned migration names (workspace ADR 0004).
- **Conflicting decisions:**
  - *axum support*: this workspace already has its own. Upstream's axum work (engine ADR 0019 upstream) is ported only where it adds behavior the workspace lacks (the malformed path/query `400 bad_request` wording, the framework-independent-file drift check). The workspace's generator structure stays.
  - *Generated model shape*: the workspace's engine ADR 0018 stands; upstream's validation and pre-checks are re-expressed on it.
  - *Template auth stack*: upstream's is taken whole (the workspace never diverged there beyond partial storage/avatar/API-key pieces), and the partial pieces are reconciled to upstream's 0.8 behavior.
- **ADR import rule:** upstream engine ADRs are imported in upstream order with the next free engine numbers (0017 validation rules → 0019, 0018 auth as template features → 0020, 0019 axum → 0021, 0020 two-factor/API keys/files → 0022). Each carries a "Formerly rok-sqlc ADR NNNN" line. No existing ADR is renumbered. The new workspace ADR records this rule, the absorption and the freeze.
- **`rok sqlc` grammar** stays owned by the engine's CLI module; rok-cli keeps forwarding tokens verbatim (AGENTS.md invariant 3). New `init` flags are added there and to `docs/cli-reference.md` together.
- **New template dependencies** (`aes-gcm`, `totp-rs`, `qrcode`, `object_store` and their peers) live only in the templates' own `Cargo.toml`s; the engine crate does not gain them. sqlx stays a dependency of `rok-data-core` only, and of the excluded template/fixture crates.
- **`regex-syntax`** becomes an engine dependency (0.5) to check `regex` rules at generation time.
- **Freeze:** after the 0.8 ticket lands, rok-sqlc gets a README banner and a final CHANGELOG entry pointing to `rok sqlc`; no code change and no new release are required there.

## Testing Decisions

- **A good test here drives the tool or the generated app from the outside**: run `rok sqlc …`, compare generated output, compile the generated crate, call the generated API over HTTP. No test reaches into engine internals to assert how a port was done.
- **Seams, highest first — no new ones:**
  1. **The `rok` binary** — rok-cli's suites spawn `CARGO_BIN_EXE_rok` with `sqlc` prepended (workspace ADR 0003). Upstream's binary-driven suites (`api_rules`, `template_drift`, the `check-arch` cases, `init --framework`) land here.
  2. **Generated-output snapshots** — the engine's expected files and `milestone-1.txt`, refreshed only with `ROK_DATA_SQLC_UPDATE_EXPECTED=1`.
  3. **Fixture compile gates** — rok-cli's `fixture_compile` suites compile the scaffolded templates and fixture apps.
  4. **Template integration tests** — each template's own `tests/` (two_factor, api_keys, media, avatars, …), ported with the template; DB-backed, and skipping with a printed note when the database URL is unset.
- **Engine library suites** (schema reading of `DROP CONSTRAINT`, `= ANY (ARRAY[...])`, `RENAME CONSTRAINT`) stay in the engine crate, beside `migration_alters`.
- **Prior art:** `crates/rok-cli/tests/cli/check_arch.rs` (architecture rules through the binary), `crates/rok-data-sqlc/tests/migration_alters.rs` (schema reading), `crates/rok-cli/tests/fixture_compile/` (template compile gates), upstream `templates/axum-api/tests/` (template behavior).
- **Every ticket's gate:** `cargo fmt --all`, `cargo clippy --workspace --all-targets -D warnings`, `cargo test --workspace --all-targets`, the module corpus, and the fixture compile gates.

## Out of Scope

- New capability beyond what rok-sqlc 0.8.0 ships.
- Changes to the runtime (`rok-data-core`, macros, facade) or to the model-crate generator (`rok-data-codegen`).
- Merging git histories; the absorption is a behavioral port.
- Deleting, archiving or transferring the rok-sqlc repository; it is frozen, not removed.
- Supporting a database other than PostgreSQL.
- rok-sqlc's `Unreleased` work after v0.8.0 (one ticket-resolution commit at the time of writing); anything there is evaluated separately.

## Further Notes

- **Precondition:** dx-first ticket 01 (reconcile `main` with `origin/main`); the ports also wait for the dx-first generated-shape tickets so new features are born in the model-centric shape.
- Upstream release prep commits (`release prep for 0.N.0`) carry docs and guide changes; the guide pages they touch (`auth.md`, `files.md`, validation) are ported into the engine's docs alongside the behavior.
- AGENTS.md's ADR counts are stale today (engine series reaches 0018, workspace 0015); the docs ticket updates them to the post-import numbers.
