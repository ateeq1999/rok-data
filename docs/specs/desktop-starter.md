# Spec: the desktop App in the monorepo template

Workspace ADR 0031 (`docs/decisions/0031-desktop-app-in-the-monorepo-template.md`),
amending ADR 0027. Tickets: `.scratch/desktop-starter/issues/`.

`rok init --template monorepo` writes `apps/desktop`, a rok-ui (GPUI) desktop
app that signs in to `apps/api` and shows its users, through the generated
Rust client.

## What ships

- **`apps/desktop`**, imported from `cargo rok-ui new --template bloc-http` at
  a pinned rok-ui release (the import commit says which), with mechanical
  alignment only: the package name `desktop`, its place in the monorepo's root
  workspace, and the API's address (`http://127.0.0.1:8080`, `apps/api`'s
  `BIND_ADDR`, overridable by an environment variable the App documents).
- **The data layer on `clients/rust`**: the repositories call the generated
  client; the starter's hand-written HTTP calls go. Validation errors still
  land on form fields (the client's error carries the field issues).
- **Auth**: sign-up, sign-in and sign-out against `apps/api`'s routes; the
  session (the bearer token) in the OS keychain through the `keyring` crate,
  read back at start, cleared at sign-out and on a 401. An unverified account
  is told to verify its email, the message `apps/api` answers.
  Tests never touch the real keychain: the store is a trait with an in-memory
  implementation, as `rok-cli`'s keychain does.
- **My account**: every signed-in user's own account (`auth().me()`: email,
  display name, whether the email is verified, role), where Home points.
- **Users**: a list (paged as the API pages it) and a detail screen, in place
  of the starter's notes. They are `apps/api`'s admin routes, so the
  navigation shows them only to an admin, and anyone else opening `/users` is
  told it needs the admin role (the owner's decision, 2026-11-02: a user who
  just signed up could not finish the walk below while the users screens
  were its only destination).
- **The shell**: rok-ui's `AdaptiveScaffold` with navigation (Home, My
  account, and Users for an admin), the Rok theme, and a light/dark toggle.

## Gates

- A `fixture_compile` test scaffolds the monorepo with the built `rok`, builds
  `apps/desktop` and runs its tests (GPUI's test context, headless). It skips
  with a printed note where GPUI cannot build (Linux without its libraries).
- A `desktop` CI job installs the libraries (`libxkbcommon-dev
  libxkbcommon-x11-dev libwayland-dev libvulkan-dev libfontconfig-dev
  libx11-xcb-dev libxcb1-dev`) and runs that test. The Rust matrix is
  unchanged.
- The template's package stays under crates.io's limit, and nothing built
  (`target/`) is embedded.

## Done means

A fresh `rok init --template monorepo` project builds `apps/desktop`; with
`apps/api` running against PostgreSQL, a user signs up, verifies, signs in,
sees their account, restarts the App still signed in, and signs out; an
admin also sees the users list and a user's detail. The README of the
monorepo says how.
