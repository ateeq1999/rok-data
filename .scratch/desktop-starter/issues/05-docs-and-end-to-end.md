# 05: docs and the end-to-end check

**Blocked by:** 03, 04
**Status:** done

The monorepo's README and AGENTS.md describe `apps/desktop` (run, build, test);
CHANGELOG `[Unreleased]`; `docs/llms.txt` and `docs/README.md` list ADR 0031
and the spec. Run the spec's Done means by hand and record the run under
`## Comments`.

## Comments

**The end-to-end run (2026-11-02, Windows 11).** `rok` built from this
branch (`cargo build -p rok-data-cli --bin rok`), then, in a scratch
directory outside the repository:

1. `rok init my-shop --template monorepo --org com.acme`: scaffolded; its
   next steps start with `rok gen`.
2. Without `rok gen`, a second fresh scaffold's `cargo build -p
   fresh-shop-api` failed with 140 errors (`cannot find users in db`,
   `cannot find queries in db`, ...): the scaffold ships `apps/api`
   without `src/db/` and the generated feature files, which `rok gen`
   writes. Ticket 03's failure was this. The monorepo's README said
   `rok init` then `cd` only, so following it failed: it now runs `rok
   gen` right after, and `rok-templates`'
   `the_monorepo_readme_starts_with_rok_gen` keeps it there.
3. `rok gen` from the root: exit 0, 87 files generated under
   `apps/api/src/`, `openapi.json` re-exported, the Clients unchanged.
4. PostgreSQL 17 in a throwaway container on port 47345; `rok migrate up`
   in `apps/api` applied its 8 migrations. `cargo build -p my-shop-api`
   built; the API ran with `DATABASE_URL`, `JWT_SECRET`,
   `TOTP_ENCRYPTION_KEY`, `BIND_ADDR=127.0.0.1:47380` and `RUST_LOG=debug`.
   Without `TOTP_ENCRYPTION_KEY` it exits at start ("environment variable
   `TOTP_ENCRYPTION_KEY` is not set"), so `rok init`'s next steps now name
   it beside `DATABASE_URL` and `JWT_SECRET`; they also name the desktop
   App, which they left out.
5. `cargo build -p my-shop-desktop`: built
   (`target/debug/my-shop-desktop.exe`).
6. A throwaway test in the scaffold's `apps/desktop/tests/` (not
   committed) drove the App's real `Api::kept`, `AuthRepositoryImpl`,
   `UsersRepositoryImpl`, `AuthBloc`, `UsersBloc` and `UserBloc` over real
   HTTP against that API, one step per process so a restart is a new
   process, the token kept in a file through a test `TokenStore` (never
   the OS keychain):
   - sign-up (`SignUpSubmitted`): `Success`, `signed_up` the email;
     signing in at once: `Failure`, "the email address is not verified
     yet".
   - verify: the log transport logged the mail with its link
     (`Verify email: http://localhost:8080/verify-email?token=...`);
     `POST /api/v1/auth/email/verify` with that token answered `200
     {"message":"email verified"}` (by curl, as the web App's page would
     send it). The database was not touched for this.
   - sign-in: `Success`, the user, the token kept. The users list as a
     plain user: `Failure`, "this needs the admin role" (the API's users
     routes are admin-only). After `UPDATE users SET role = 'admin'` in the
     database: the list `Success` (total 1, the user in it), the detail
     `Success` (the same id).
   - restart (a new process): the kept token restored into the session,
     `ProfileRequested` answered the user, the list and the detail again
     `Success`.
   - sign-out (a new process): the bloc's user `None`, the session's token
     gone, the kept token deleted; a further restart starts signed out.

   Signing out is the App forgetting the token (the API has no sign-out
   route): the token stays valid on the API until it expires.
7. The API stopped (its PID only) and the container stopped; `cmdkey
   /list` still holds one `rok` entry.

**Not done: the GUI.** The window itself was not driven: no GUI could be
operated from this session. The bloc steps above are what the views send
and read; ticket 04's `tests/shell/` covers the rendering against a mock
API.

**Docs.** The monorepo's README (start with `rok gen`; the desktop App:
run, build, test, `ROK_API_URL`, verifying an account, the keychain
session, the Linux keyring, making an admin for the users screens),
AGENTS.md, `apps/desktop/README.md`, the templates' monorepo guide (no
longer says sign-up, the keychain session and the users screens are still
to come), CHANGELOG `[Unreleased]`, `docs/llms.txt` and `docs/README.md`
(ADR 0031 and this spec), getting started and the CLI reference (the
next steps). The stale reqwest 0.13 note in monorepo-template ticket 07
is corrected.
