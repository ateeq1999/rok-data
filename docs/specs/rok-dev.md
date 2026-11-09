# Spec — `rok dev`: one command for a project's stack

Status: ready-for-agent · Implements workspace ADR 0034 · Source: Karibu Pass's `scripts/dev.sh` (between its tickets 22 and 23)
Interview outcome in one line: a new member, rok-dev, owns `rok dev`, `rok run`, `rok doctor` and `rok port`. It starts a project's Runnables from `rok.toml`'s `[dev]` tables in dependency order, reuses what's already up, owns what it starts, waits on readiness checks that can't break a slow server, applies migrations first, boots or recovers the device, passes the API address the device needs, and checks the environment.

## Problem Statement

Running a Monorepo project locally means starting the compose database, the API with its `.env`, the web App and the mobile App on an emulator, each in its own shell. Karibu Pass wrote a script for it and hit, in turn:
- **Readiness:** a readiness probe whose 2 s timeout aborted Vite's slow first render.
- **Orphans:** an API and a Vite left running when the script was killed.
- **API address:** the emulator needing `10.0.2.2` instead of `localhost`.
- **A hung AVD:** recovering it meant removing a lock in a folder the `.ini` names, not the one its name suggests.
- **Kotlin:** a build that fails only when the project and the pub cache are on different drives.
- **Ports:** a port still held by a process nobody remembered starting.

## Solution

`rok dev` starts every Runnable `rok.toml` names, or the ones you list.
- **Order:** builds in parallel; start-up and readiness in dependency order.
- **Reuse:** anything already running is reused.
- **Logs:** they're multiplexed (`api │ …`).
- **Ctrl-C:** stops what `rok dev` started, and only that.
- **Migrations:** applied or reported before the API starts.
- **Devices:** `--device` picks or boots one, and the mobile App gets the API's address for it.
- **Other commands:**
  - `rok run <name>` runs one Runnable in the foreground with its `.env`.
  - `rok doctor` checks the toolchains and the known traps.
  - `rok port list` / `rok port free <port>` find and free what holds a port.

## User Stories

1. As a developer, I want `rok dev` to start the database, the API, the web App and the mobile App, so that one command gives me a working stack.
2. As a developer, I want `rok dev api web` to start only those (and what they depend on), so that I run what I'm working on.
3. As a developer, I want a Runnable that's already up to be reused, not started twice, so that `rok dev` works alongside an API I started myself.
4. As a developer, I want Ctrl-C, a closed terminal or a killed `rok dev` to stop everything it started, so that no API or Vite is left holding a port.
5. As a developer, I want each Runnable's output prefixed with its name, so that I can read one stream.
6. As a developer, I want readiness checks that wait patiently (`port:`, `http:` with no short abort, `log:`), so that a slow first render doesn't fail the start.
7. As a developer, I want pending migrations applied (or reported) before the API starts, so that I don't find out from query errors.
8. As a mobile developer, I want `--device android:sp` to reuse or boot the emulator and wait until it's ready, so that `flutter run` finds it.
9. As a mobile developer, I want a hung emulator cleared, its stale lock included, so that the next boot works.
10. As a mobile developer, I want the App to get the API address for its device, so that I never hardcode `10.0.2.2`.
11. As a developer, I want the first builds to run at once, with a progress line each, so that a 12-minute Gradle build overlaps the API's.
12. As a developer, I want `rok run api` to run one Runnable in the foreground with its `.env`, so that I don't export variables by hand.
13. As a developer, I want `rok doctor` to check Docker, cargo, bun, Flutter, the Android SDK and the AVD and name each fix, so that a new machine is ready quickly.
14. As a developer on Windows, I want `rok doctor` to spot the Kotlin "different roots" failure and offer the fix, so that I don't lose an afternoon to it.
15. As a developer, I want `rok port list` and `rok port free 3001`, so that I can find and stop what holds a port without platform-specific commands.
16. As a script author, I want `rok dev --json` to print one event per line, so that I can wait for readiness from another tool.

## Implementation Decisions

- **The member:** a repo, rok-dev, with a `rok-dev-cli` crate and a `rok-dev` binary. rok-v2 links it and routes `dev`, `run`, `doctor` and `port` to it (ADR 0032's Subcommand CLIs).
- **Config:** `[dev.<name>]` tables in `rok.toml` (see ADR 0034).
  - `run` is an argv array (no shell); `compose` points at a compose file; `dir` is the working directory.
  - `ready` is `port:N`, `http:<path>` (polled with a generous per-request timeout, never aborting an in-flight render) or `log:<regex>`.
  - `depends` lists other Runnables; `migrate` is `apply`, `warn` (the default) or `off`, for a Runnable that is a rok-data App; `device` is for a mobile App; `env` sets variables for the Runnable over its `.env` (the templates set the API's `CARGO_TARGET_DIR` to `target/dev`, so `cargo test` can build while it runs on Windows).
  - Each Runnable's `.env` comes from its `dir`, and the environment wins.
  - An unknown key, a cycle in `depends` or a missing `run`/`compose` is an error naming it.
- **Supervision:**
  - **Ownership:** children are created in a Job Object (Windows, kill-on-close) or a new process group (Unix), and on exit, signal or the parent's death the tree is stopped.
  - **Reuse:** a Runnable whose `ready` already passes before start is reused and left running.
  - **Logs:** output is line-prefixed with the name, coloured, and aligned; `--json` prints one event per line (`started`, `building`, `ready`, `exited`).
- **Order:** every build step starts at once; a Runnable's process starts when its `depends` are ready; progress lines show long builds.
- **Migrations:** for a Runnable with `migrate`, rok-dev uses rok-data-cli's library to check (and with `apply`, run) the App's pending migrations before starting it.
- **Devices:**
  - **`android:<avd>`:** reuses a booted emulator (`adb devices`); otherwise runs `emulator -avd <avd> -no-snapshot-load` and waits for `sys.boot_completed`. A hung instance is stopped and its `multiinstance.lock` removed from the directory its `.ini`'s `path` names.
  - **`ios[:<name>]`:** uses `xcrun simctl boot`.
  - **`lan` / `usb`:** a physical device, through `flutter devices`.
- **API address:** for a mobile Runnable that depends on the API, rok-dev passes `--dart-define=API_URL=<origin>`: `10.0.2.2` on the emulator, `127.0.0.1` on the simulator, the LAN IP for a phone, with the API's port.
- **`rok run <name>`:** the one Runnable in the foreground, with its `.env` and its `dir`, without its dependencies.
- **`rok doctor`:**
  - **Checks:** Docker running, cargo, bun, Flutter, the Android SDK with `adb` and `emulator`, the configured AVD, `xcrun` on macOS, and the `[dev]` ports.
  - **Known traps:** the Kotlin incremental "different roots" failure (the project and the pub cache on different drives; `--fix` writes `kotlin.incremental=false` to the App's `gradle.properties`), and the Gradle 8.14 / AGP 8.11 / Kotlin 2.2 deprecations.
  - **Output and exit code:** each check is ok, warn or fail with its fix, and any fail exits non-zero.
- **`rok port`:**
  - **`list`:** shows the process holding each `[dev]` port, or a given one.
  - **`free <port>…`:** stops it and its tree after asking, or with `--yes`. It refuses a process it can't identify.
- **Templates:** each template ships its `[dev]` tables; the Monorepo's cover db, api, web and mobile.

## Testing Decisions

- **Config:** parsing and validation are unit-tested (each `ready` form, `depends` cycles, unknown keys).
- **Supervision:**
  - **Integration tests:** they start small helper binaries built for the tests (one that listens on a port after a delay, one that prints a line, one that ignores SIGTERM) and check readiness, reuse, ordering, log prefixes, `--json` events, and that killing `rok dev` leaves no child running — on Windows and Unix in CI.
- **Migrations:** tested against a throwaway PostgreSQL, skipping without one.
- **Devices and doctor:** the parts that read files (the AVD `.ini` → lock path, `gradle.properties`, the drive check) are unit-tested; the parts that call `adb`, `emulator`, `xcrun` or `flutter` go through a small runner trait faked in tests.
- **Ports:** `port list` and `port free` are tested against a helper that holds a port.

## Out of Scope

- **Production process management:** systemd, containers for deploy.
- **Hot reload of the API:** cargo-watch-style rebuilds; a later ticket.
- **Remote devices and device farms.**

## Further Notes

- **The `[dev]` table in rok.toml:** rok-data's project rule (starter-feedback ticket 24) reads `rok.toml`'s `[apps]` and leaves other sections alone; a test there should keep it so.
- **Repo creation:** the GitHub repo `ateeq1999/rok-dev` is the maintainer's to create (public repo creation is a manual step, as for rok-docs).
