# ADR 0034: rok-dev runs a project's stack

Date: 2026-11-07 · Status: accepted (follows ADR 0032)

## Context

A project built on the Monorepo template needs its database, API, web App and mobile App running to work on it. Karibu Pass wrote a `scripts/dev.sh` for that, and each step was something rok could own:
- **Starting things:** the compose database, the API with its `.env`, Vite, and the Flutter App on an emulator.
- **Readiness:** checks Vite's slow first page can survive.
- **Migrations:** applied before the API serves.
- **Clean-up:** children that a killed script left orphaned.
- **Devices:** the emulator's `10.0.2.2` API address, and recovering a hung AVD.
- **Environment traps:** a Kotlin build failing across drives.
- **Ports:** freeing a port something still holds.

None of this is data tooling, and none of it is a file tool: it supervises long-running processes.

## Decision

- **A new member, rok-dev** (a repo with a `rok-dev-cli` crate and a `rok-dev` binary), linked into `rok` as rok-tools is (ADR 0032). It owns the words `rok dev`, `rok run`, `rok doctor` and `rok port`.
- **Settings live in `rok.toml`**, one `[dev.<name>]` table per **Runnable** (an App, or a container an App needs):
  - `run` (or `compose`)
  - `dir`
  - `ready`: `port:N`, `http:<path>` or `log:<regex>`
  - `depends`
  - `migrate`: `apply`, `warn` or `off`
  - `device` for a mobile App

  Each Runnable's `.env` is read from its directory.
- **rok-dev owns what it starts.** Children go in a Job Object on Windows or a process group on Unix, so a hard stop takes the tree down. What was already running (its `ready` check passes before start) is reused, never stopped.
- **The API's address follows the device:** `10.0.2.2` on the Android emulator, `127.0.0.1` on the iOS simulator, the LAN IP for a phone. It is passed as `--dart-define=API_URL`.
- **Builds run in parallel.** `depends` orders only start-up and readiness.
- **The templates ship their `[dev]` tables.**

## Considered Options

- **Inside rok-tools:** its Tools are single-purpose and short-lived, behind cargo features; supervising processes, devices and logs is a different kind of code with different dependencies.
- **Inside rok-data-cli:** it is data tooling.
- **Inside rok-v2:** ADR 0032 keeps it to routing.
- **A generated `scripts/dev.sh` per template:** it would not own its children, and it differs per shell and OS.

## Consequences

- One more member repo and release train; rok-v2's minor follows rok-data's, and rok-dev versions on its own like rok-tools.
- `rok.toml` gains a `[dev]` section that rok-data's project rule must leave alone.
