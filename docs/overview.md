---
title: rok-data overview
description: What rok-data is, the problem it solves, its crates at a glance, and the one naming trap (rok-cli vs rok-data-cli).
updated: 2026-10-29
kind: overview
source: README.md, docs/architecture.md, AGENTS.md
---

# rok-data overview

**One schema. All changes.**

rok-data is PostgreSQL data tooling for Rust, consolidated from the rok
ecosystem — four earlier repositories (`rok-db`, `rok-fluent`,
`rok-orm`, `rok-sqlc`) became one Cargo workspace with one runtime, one
generator pair and one CLI. The project's schema directory is the only
source of truth (workspace ADR 0010): plain `*.sql` files are the
schema as it is now, and the `NNNN_name` migration pairs beside them
are the history — `rok init` scaffolds it, `rok gen` turns it into a
typed data layer and an actix-web or axum API, and `rok gen check`
fails the build when the generated code goes stale.

## What it gives you

- **Typed models, one derive** — `#[derive(Model)]` expands to typed
  column constants, relations, named scopes and the
  `Model`/`ModelWrite` trait impls. Reads chain; writes are row-level.
- **Migrations you can trust** — numbered `up`/`down` pairs, a
  checksummed journal (`_rok_data_migrations`), advisory-locked and
  transactional, drift detection with the states
  `applied | pending | edited | unknown`.
- **Generated APIs, two frameworks** — actix-web or axum API features
  and complete starter templates, with utoipa OpenAPI and the
  `check-arch` layout gate.
- **Machine contracts** — `--json` shapes, exit codes, `--dry-run`,
  check modes; agent skills under `.agents/skills/rok-data*/`.

## The crates

| Crate | Role | Page |
| ----- | ---- | ---- |
| `rok-data` | the facade applications depend on | site `/crates/rok-data` |
| `rok-data-core` | `Db`, `Expr`/`Select`, migrations, introspect | site `/crates/rok-data-core` |
| `rok-data-macros` | the derives + `#[rok_data::test]` | site `/crates/rok-data-macros` |
| `rok-data-codegen` | whole-crate generator + migration diff | site `/crates/rok-data-codegen` |
| `rok-data-sqlc` | resource-module engine, init, check-arch | site `/crates/rok-data-sqlc` |
| `rok-templates` | the actix, axum and monorepo starter templates — [their own repository](https://github.com/ateeq1999/rok-templates) (workspace ADR 0032) | its README |
| `rok-data-cli` | the `rok-data` binary, linked into rok-v2's `rok` | site `/crates/rok-cli` |

## Scope of this documentation

These docs (and the TanStack Start site under `site/`) cover the
rok-data workspace only. The rok family's other members — the
platform's `rok-cli` execution engine, `rok-orm`, `rok-migrate`,
`rok-http`, `rok-auth`, `rok-utils` — live in the sibling repository
[github.com/ateeq1999/rok](https://github.com/ateeq1999/rok) and have
their own docs. Where rok-data came from them (per-repository migration
notes, the capability ledger), see
[`docs/migration-guide.md`](migration-guide.md) and
[`docs/feature-matrix.md`](feature-matrix.md).

## The one naming trap

The crates.io crate `rok-cli` changes hands at 0.7.0 (workspace ADR 0032):

- **up to 0.6.1** it is the old execution-engine CLI ("Run One. Know
  All."), now archived;
- **from 0.7.0** it is [rok-v2](https://github.com/ateeq1999/rok-v2),
  whose `rok` binary links this workspace's CLI.

This workspace's CLI is the crate `rok-data-cli`, binary `rok-data`.
Everything documented in [`docs/cli-reference.md`](cli-reference.md)
runs as `rok …` through rok-v2 or as `rok-data …` on its own.
