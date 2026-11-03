# Spec — The starter, fixed from a project's first build

Status: ready-for-agent · Implements workspace ADR 0033 · Source: Karibu Pass's build notes ("What the rok starter cost us", 2026-11-03, commits `fa369ae`, `bf9b676`, `32651f6`)
Interview outcome in one line: fix the rok side of what scaffolding a Monorepo project cost — a CI gate that installs `rok` in seconds, `-w` for a Template's API App, Modules left out with `--without`, one source for design tokens, kits inside their Apps, and a local setup that works on the first run — and leave the project's own product work (phone sign-in, its tickets) to it.

## Problem Statement

A team scaffolded the Monorepo template with `rok init` and built three Apps on top of it. Before writing product code they spent time on the template itself:

- **The API framework was fixed:** the Monorepo template's API App is axum only, so a team that had chosen actix ported the OpenAPI example and two tests by hand, and has to re-check that port on every rok upgrade.
- **The CI gate was slow, and as shipped it doesn't work:** each template's CI runs `cargo install rok-data-cli --locked`, which builds from source on every run, then calls `rok`, a binary that package does not install.
- **Init flags were unclear:** `-w` can't be combined with `-t`, and the usage error doesn't say why; `-s` seeds a root `schema/` the Monorepo template never reads.
- **Parts they didn't need had to be removed by hand:** API keys, media library, audit log, admin users, TOTP and theming, across API, web and mobile and the template's own tests. The TOTP issuer named the crate (`karibupass-api`).
- **One design system existed in three copies:** the UI kits shipped as packages broke bun's `file:` dependency, and once moved into each App the tokens lived in TypeScript, Dart and Rust with nothing keeping them in step.
- **Template files weren't formatted to current tools,** so `dart format` rewrote 57 of them, and CRLF line endings broke scripted edits on Windows.
- **Local setup had gaps:**
  - The API doesn't read `.env`.
  - No dev account can sign in without setting `email_verified` in Postgres by hand.
  - Postgres's port 5432 is hard-coded.
  - `tsr generate` crashes.
  - The web tests need a Node flag on Node 26.
- **The mobile screens could not be checked against the design** without a running API, so the team wrote, then deleted, a render test of its own.

## Solution

**CI:** a scaffolded project's CI installs the pinned `rok` release binary in seconds and runs the same checks it runs today.

**Init:** `rok init kp -t monorepo -w actix_web` gives an actix API App, and `--without api-keys,media,audit-log,admin,totp,theming` leaves out Modules the project doesn't need, with the rest still compiling and passing its tests. `-s` seeds the schema directory `rok gen` reads.

**Design tokens:** one `tokens.json` generates the web, mobile and desktop tokens, and CI fails if they drift. Each UI kit lives in its App's own source.

**Templates:** they ship formatted, with LF line endings enforced.

**Local setup:**
- the API reads `.env` in debug builds
- a dev seed creates verified accounts
- Postgres's port is a setting
- the web App can regenerate its route tree on its own
- the web tests run on any Node

**Visual check:** one golden test checks a mobile screen against its design without an API.

## User Stories

1. As a developer starting a project, I want `rok init` to scaffold the Monorepo template with an actix API App, so that the framework my team chose comes generated and kept current by rok, not ported by hand.
2. As a developer, I want `-w` combined with a single-app template to explain that the template is one framework already and name the template to use instead, so that I don't decode a usage error.
3. As a developer, I want `rok init … --without api-keys,media` to leave those Modules out, so that I don't unwire them by hand across three Apps.
4. As a developer, I want a project with Modules left out to compile and pass its tests straight from `rok init`, so that leaving a Module out is safe.
5. As a developer, I want `rok init` to refuse to leave out a Module that another kept Module needs, naming both, so that I never get a project that doesn't build.
6. As a developer, I want a Blueprint to remember the API framework and the Modules left out, so that every project created from it starts the same.
7. As a developer, I want `-s schema.sql` to land in the schema directory `rok gen` reads, and the init summary to say where, so that my seed schema is used.
8. As a developer, I want my project's CI to install `rok` in seconds instead of building it, so that the drift check doesn't slow every push.
9. As a template maintainer, I want the pinned `rok` version in the template's CI checked against `rok-data.pin`, so that the two can't disagree.
10. As a developer, I want the TOTP issuer to be a setting that defaults to my project's name, so that authenticator apps show my product, not my crate.
11. As a designer-developer, I want one `tokens.json` to produce the web, mobile and desktop tokens, so that the three Apps can't drift apart.
12. As a developer, I want CI to fail when a generated tokens file is stale, so that a token change reaches every App.
13. As a developer, I want each UI kit in its App's own source, so that package-manager quirks like bun's `file:` copies can't break the build.
14. As a developer, I want template files formatted with the tools' pinned versions, so that my first format run doesn't rewrite files I never touched.
15. As a developer on Windows, I want the project to enforce LF line endings, so that scripted edits and byte-compared checks behave the same as on Linux.
16. As a developer, I want the API to read `.env` when I run it in debug, so that `cargo run` works without exporting variables.
17. As a developer, I want a dev seed that creates a verified admin and a verified ordinary user and prints their credentials, so that I can sign in on the first run.
18. As a developer, I want Postgres's host port to be a setting, so that an existing local Postgres doesn't collide with the project's.
19. As a developer, I want a script that regenerates the web route tree with the router's own CLI version, so that I don't need a full build to add a route.
20. As a developer, I want the web tests to run on the current Node without me adding flags, so that a Node upgrade doesn't break `bun run test`.
21. As a mobile developer, I want a golden test of a screen with the real fonts, so that I can check the screen against its design without a running API.

## Implementation Decisions

- **CI install** (every template's workflow):
  - **Install:** replace `cargo install rok-data-cli --locked` with downloading the rok release tarball for `x86_64-unknown-linux-musl` from rok-v2's GitHub release, checking it against its `.sha256`, and putting `rok` on the PATH.
  - **Pin:** a `ROK_VERSION` variable in the workflow pins the release.
  - **Check:** a rok-templates test checks that `ROK_VERSION` is a rok release linking the rok-data minor in `rok-data.pin`.
  - **Commands:** each template's checks stay as they are (`rok sqlc g --check` and `rok sqlc check-arch` for single-app templates, `rok gen check` for the Monorepo).
- **`-w` with a template:**
  - **Combining:** `--api-framework` combines with a Template that has an API App and picks that App's framework (`actix_web`, `axum`, `grpc`; default `axum`).
  - **Source:** the Monorepo's API App is drawn from the matching single-app Template, so the framework variants are not kept by hand a second time.
  - **Single-app templates:** for a single-app Template, `-w` is an error that names the Template to use instead.
- **Modules** (workspace ADR 0033):
  - **Catalog:** the rok-templates catalog declares each Module of a Template (its name, its files per App, its wiring points, the Modules it needs).
  - **Flag:** `--without <MODULE,…>` drops those files and wiring lines at init, and refuses to drop a Module a kept one needs.
  - **First Modules:** `api-keys`, `media`, `audit-log`, `admin`, `totp`, `theming`.
  - **Capabilities:** Capabilities are not Modules.
  - **Blueprint:** a Blueprint records the API framework and the Modules left out.
- **`-s` seeds** the schema directory the project's `rok.toml` names for the App `rok gen` generates from (in the Monorepo, the `api` App's), and the init summary prints the path. Single-app templates keep `schema/0001_init.sql`.
- **TOTP issuer:** a `TOTP_ISSUER` setting, defaulting to the project name without a trailing `-api`.
- **Design tokens:**
  - **Source:** the Monorepo template ships one `tokens.json`.
  - **Generator:** `rok tools tokens` (rok-tools) writes the web `styles.css` tokens, the mobile Dart tokens file and the desktop Rust tokens from it; `--check` fails when any is stale, and the template's CI runs it.
  - **Kits:** each UI kit moves into its App's own source; no kit ships as a package.
- **Formatting and line endings:** template files are formatted with the pinned `dart format` and Prettier versions, the gates fail on an unformatted template file, and every Template ships `.gitattributes` with `* text=auto eol=lf`.
- **Local setup:**
  - **`.env`:** the template APIs load `.env` with dotenvy in debug builds only.
  - **Dev seed:** a `seed` binary creates a verified admin and a verified ordinary user and prints their credentials; `REQUIRE_VERIFIED_EMAIL` stays `true` by default.
  - **Postgres port:** `compose.yaml` maps `"${POSTGRES_PORT:-5432}:5432"`, `.env.example` sets `POSTGRES_PORT` and uses it in `DATABASE_URL`, and the stale `actix_api` database name in the Monorepo API's `.env.example` is fixed.
- **Web App:**
  - **Route tree:** `@tanstack/router-cli` is pinned to the router's version and a `routes` script runs `tsr generate`; the web gate runs it and fails if `routeTree.gen.ts` changes.
  - **Tests on Node 26:** the `test` script sets `NODE_OPTIONS=--no-experimental-webstorage`, and the gates pin the Node version.
- **Mobile golden test:** the Monorepo's mobile App ships one golden test of its sign-in screen, with the OFL fonts bundled so it renders the same on every machine.

## Testing Decisions

- **Gates:** they test what a user's project gets, through `rok init` and the project's own checks, not template internals.
- **Modules:**
  - **Builds:** for every Template with Modules, the gates build and test the default and the all-out combination.
  - **Engine tests:** `--without` with an unknown Module, and with a Module a kept one needs, are usage errors with their own tests.
- **CI install:** the `ROK_VERSION` check is a rok-templates unit test. One gate run with the workflow's install step confirms the download and checksum work.
- **Tokens:** `rok tools tokens` has unit tests over a sample `tokens.json` (each output's content), and a `--check` test that fails on a hand-edited output.
- **Init:** `-w` with `-t monorepo`, `-w` with a single-app template, and `-s` with the Monorepo are init tests in rok-data, beside the existing ones.
- **Dev setup:** the seed binary is tested against the gates' database (it creates two accounts that can sign in). The route-tree and formatting checks are part of the web and mobile gates.

## Out of Scope

- **Karibu Pass's product work:** phone and SMS sign-in (its ADR-0006), its tickets, removing its sample data, its repo-wide reformat and its own visual-check scripts. A phone sign-in Module may come later.
- **Restyling primitives:** restyling shadcn primitives the designs don't use.
- **Command changes:** moving the single-app templates to `rok.toml` and the front-door commands.
- **Presets:** presets for `--without` lists (ADR 0033 leaves them for later).
- **Web screenshots:** a web screenshot script.

## Further Notes

- **Already fixed:** the `--org` error message the notes ask about already explains the format in current rok ("not a reverse-domain name like `com.example`: two or more dot-separated segments…"); it needs no change.
- **Ordering:** ticket 01 (the CI install) starts now. The rest follows the shipping of the gRPC Template (rok-data 0.8.1, the rok-templates release and `rok init --template grpc`), and goes with the Monorepo `--apps`/`--clients` flags, which are the same kind of init-time choice.
