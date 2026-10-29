# ADR 0028: rok-data stays Postgres-shaped; Servers are per user

Date: 2026-10-29 · Status: accepted

## Decision

- **The project manager is not rok-data's.** A registry of a user's
  projects (any stack: Rust, Node, Go, Flutter, TanStack, rok) and
  `rok new` for stacks rok does not generate know nothing about
  PostgreSQL, so they live in a separate workspace that `rok` forwards
  to verbatim, as `rok tools …` does (ADR 0026). Foreign stacks are
  created by their own tools (`npm create`, `flutter create`, `go mod
  init`) and then registered; rok stacks go through `rok init`. The
  registry is a local per-user file, readable with no database running.
  Deleting a project unregisters it; removing its folder is a separate,
  confirmed `--purge`.

- **Servers are per user, not per project.** A **Server** is a named
  PostgreSQL connection registered once (`rok server add local --url …`)
  in the user's config directory (`servers.toml`), its password in the
  OS keychain and never in a plain file. `rok db` commands that act on a
  server (creating, dropping, querying databases) take `--server`. A
  project's `DATABASE_URL` keeps working unchanged and stays the default
  for the commands that act on the project's own database. Where no
  keychain exists (headless Linux, CI), the password comes from
  `ROK_SERVER_<NAME>_PASSWORD`, and `server add` says so. Dropping a
  database asks for its name to be typed (`--yes` for scripts) and
  refuses the one the project's `DATABASE_URL` names unless `--force`;
  `db query` runs in a transaction that commits only on success.

## Considered Options

- **The registry inside rok-cli**: the CLI would stop being PostgreSQL
  data tooling (the glossary's first line), and the registry's stacks
  would pull their concerns into this workspace.
- **The registry as a Postgres table**: listing projects would need a
  running server.
- **Server credentials in `rok.toml`**: a project file is committed; a
  server's password must not be.
