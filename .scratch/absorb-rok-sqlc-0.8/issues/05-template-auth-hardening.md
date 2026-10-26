# 05: Template auth hardening (rok-sqlc 0.6)

**What to build:** Scaffolded projects (actix and axum) get upstream 0.6's auth: `AuthUser` checks the database on every request; a versioned `auth` migration; `JWT_SECRET` under 32 bytes stops startup; wiring through `app::configure(cfg, &AppState)`.

**Blocked by:** dx-first 03

**Status:** done (actix); axum moved to 07

- [x] actix template (axum: ticket 07); generated code uses the model verbs
- [x] Template integration tests (auth, login, sign-up, account) ported
- [x] Fixture compile gates green; CHANGELOG breaking entries (env var, migration, wiring)

**How it shipped:**
- **Hand-written files:**
  - Existing ones were three-way merged with `git merge-file` (base upstream v0.5.0, ours the workspace, theirs v0.6.0).
  - New ones were copied from v0.6.0: `features/{auth,account}`, `shared/{security,mail,rate_limit}`, `queries/*.sql`, and the integration tests.
- **Conflicts resolved in the workspace's favour:** README, AGENTS.md, Cargo.toml.template and `shared/mod.rs`. The workspace's own workflow wording stays (truth in `schema/`, `rok migrate`, `rok gen --migration`, `rok sqlc g`).
- **Schema:**
  - `users.token_version` is added in the truth's `schema/0001_init.sql`.
  - `schema/0002_auth.sql` holds `auth_tokens` and `outbound_mail`.
  - `rok gen --migration auth` staged `migrations/0002_auth.{up,down}.sql`; upstream's single-file migration isn't used.
- **Config:** `queries = ["queries/"]`, the new `exclude`, `managed_columns` and `hidden` entries, and `[api.rules.users]` (deferred from ticket 02).
- **Model verbs:** `AuditLogEntry::create` and `User::create` replace upstream's `write::create`, which is deprecated here.
- **check-arch:** the `cross-feature-query` rule, applied as upstream's patch, plus its CLI test. The rule 4 text is updated in the architecture guide and both templates' AGENTS.md.
- **Guide:** `docs/guide/auth.md` is ported, with its "add a token purpose" steps rewritten for `rok gen --migration`; I verified that it stages DROP and ADD of the check.
- **Verified against Postgres:** a template made with `init --template actix` + `g` runs 91 tests (account 3, auth 5, email_change 5, invitations 3, login 6, mail 7, password 11, sign_up 8, users 8, …), none skipped.

**Why axum moved:** upstream wrote the axum template's auth in 0.7, together with the actix hardening. Porting it there, from v0.7.0's `templates/axum-api`, avoids hand-writing an axum version twice.
