---
title: docs/ — the map
description: What every file under docs/ is for, the reading order, and how to hand pages to a coding agent.
updated: 2026-10-29
kind: index
---

# docs/ — the map

This directory is the workspace documentation. The root
[`README.md`](../README.md) is the front page; this map is the table of
contents for everything else.

## Read in this order

| # | Page | What it is |
| - | ---- | ---------- |
| 1 | [`overview.md`](overview.md) | what rok-data is, its crates at a glance, the scope of these docs, and the one naming trap |
| 2 | [`installation.md`](installation.md) | the CLI (`rok`), the runtime crate (`rok-data`), platform notes |
| 3 | [`getting-started.md`](getting-started.md) | the front door (`rok init` / `rok gen`) and the older paths behind it |
| 4 | [`coding-agents.md`](coding-agents.md) | the machine contracts (JSON, exit codes) agents drive the CLI with |
| 5 | [`cli-reference.md`](cli-reference.md) | every command, flag, output and exit code — **contract** |

## Reference

| Page | What it is |
| ---- | ---------- |
| [`architecture.md`](architecture.md) | crate responsibilities, dependency rules, migration-system layers |
| [`feature-matrix.md`](feature-matrix.md) | what exists where, mapped from the four source repositories |
| [`pagination.md`](pagination.md) | paging, cursors, chunking, streaming — the wire shapes too |
| [`migration-guide.md`](migration-guide.md) | moving a project from rok-db / rok-sqlc / rok-fluent / rok-orm, and the 0.3.0 / 0.4.0 upgrades |
| [`benchmarks.md`](benchmarks.md) | the builder-effort bench, honestly scoped |

## Agent-facing

| Page | What it is |
| ---- | ---------- |
| [`llms.txt`](llms.txt) | the machine index of this whole tree (one line per file) |

Every *working* page (the tables above) opens with YAML front matter and
an **Agent brief** — a compact block you can paste into a coding agent
before the body, so the page needs no surrounding conversation to be
usable. Pages are kept self-contained: facts cite workspace ADRs and
crate paths by repository-relative location, never by bare "see above".

Three pages carry compile-checked Rust samples —
[`getting-started.md`](getting-started.md),
[`migration-guide.md`](migration-guide.md), [`pagination.md`](pagination.md).
Each sample block is marked `<!-- source: <path> -->` and is held,
verbatim (modulo indentation), against the compiled file the marker
names by `crates/rok-data/tests/docs`. Do not edit those samples in the
page; edit the source file and re-mark.

## Records (read, don't rewrite)

| Path | What it is |
| ---- | ---------- |
| [`decisions/`](decisions/) (0001–0030) | workspace ADRs — every deliberate cross-crate choice; the single source for "why" |
| [`migration-audit.md`](migration-audit.md) | what was audited out of the four source repositories before consolidation |
| [`migration-plan.md`](migration-plan.md) | the original consolidation plan |
| [`unified-pipeline-plan.md`](unified-pipeline-plan.md) | the 0.2.0 pipeline plan (schema-dir truth, front door, templates) |
| [`specs/`](specs/) | per-release specs (0.3.0 model-shaped layer, 0.4.0 everyday verbs, the sqlc absorption, input validation, 0.5.0 production templates) |
| [`research/toasty-patterns.md`](research/toasty-patterns.md) | the toasty study behind With-loading's shape |

The engine's own ADR series and guides live outside this directory, in
[`crates/rok-data-sqlc/docs/`](../crates/rok-data-sqlc/docs/) — when a
page says "engine ADR NNNN", that is where it is. The actix and axum
templates' guide and recipes are in
[`crates/rok-templates/docs/guide/`](../crates/rok-templates/docs/guide/).
