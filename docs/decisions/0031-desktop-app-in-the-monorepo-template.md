# ADR 0031: a desktop App in the monorepo template

Date: 2026-10-30 · Status: accepted (amends ADR 0027)

## Decision

- The monorepo template gains a fourth App, **`apps/desktop`**: a desktop
  app built with [rok-ui](https://github.com/ateeq1999/rok-ui) (GPUI), beside
  `apps/api`, `apps/web` and `apps/mobile`, on the same API contract.

  ```text
  apps/desktop/    rok-ui's bloc-http starter, its data layer on clients/rust
  clients/rust     generated from apps/api's openapi.json (ADR 0027)
  Cargo.toml       workspace members: apps/api, apps/desktop, clients/rust
  ```

- **Imported, then aligned, like the web App.** `apps/desktop` starts as
  what `cargo rok-ui new --template bloc-http` writes, at a pinned rok-ui
  release: its BLoC structure (data, features, thin routes), its `Session`
  and session guard, its sign-in feature. The import is mechanical; the
  changes that follow are named in the spec, so the App stays recognizable
  to anyone who knows rok-ui's templates.

- **The data layer is the generated Rust client.** The repositories call
  `clients/rust` (ADR 0027's `reqwest` crate), not hand-written HTTP, so a
  change to the API's contract reaches the desktop App through `rok gen`
  as it reaches the web App through the TS client. The App waits for the
  Rust client rather than shipping a hand-written layer to replace later.

- **The first version: auth and one resource.** Sign-up, sign-in and
  sign-out against `apps/api`, the session kept in the OS keychain, a
  signed-in shell (navigation, the Rok theme with a light/dark toggle), and
  the users list and detail. Further screens follow `apps/web`'s, as the
  API serves them.

- **Gated in CI by its own job.** A `desktop` job, like the `web` job,
  installs GPUI's Linux libraries, scaffolds the monorepo, builds
  `apps/desktop` and runs its tests headless (GPUI's test context, no
  window). The Rust matrix stays as fast as it is; the job's
  `fixture_compile` test runs locally too.

## Considered Options

- **rok-ui's `http` module as the data layer, switched later**: the App
  would ship a hand-written API layer only to replace it, and its error
  shapes would drift from the client's.
- **The `bloc-http-workspace` starter (one crate per feature)**: closer to a
  large app, but the starter's job is to be read in an afternoon; a project
  splits crates when it grows.
- **Building it in the Rust matrix**: GPUI's build is slow and needs system
  libraries on Linux; every unrelated change would pay for it.

## Consequences

- The monorepo's root `Cargo.toml` lists three members, and `apps/desktop`
  pins the rok-ui and gpui versions it was imported at.
- The desktop App is blocked by ADR 0027's ticket 07 (the Rust client) for
  everything past the import.
- rok-ui becomes a dependency of a rok-data template, not of any rok-data
  crate: the workspace's own crates never depend on it.
