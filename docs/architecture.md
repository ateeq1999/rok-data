# Workspace architecture

`rok-data` is a Cargo workspace that consolidates four repositories behind
one runtime, one generator pair, and one CLI.

```
┌────────────────────────────────────────────────────────────┐
│                        rok (CLI crate)                     │  ← the only user-facing
│  init  gen  generate  migrate  db  schema  sqlc           │    binary in the workspace
└───────┬──────────────┬──────────────────┬──────────────────-┘
        │              │                  │
        ▼              ▼                  ▼
┌───────────────┐ ┌─────────────┐ ┌────────────────────┐
│ rok-data-     │ │ rok-data-   │ │ rok-data-sqlc      │
│ codegen       │ │ core        │ │ (engine, library)  │
│ model-crate   │ │ runtime: Db, │ │ schema parser,     │
│ generator     │ │ Expr/Select,│ │ resource-module    │
│               │ │ migrations, │ │ generator, api     │
│               │ │ introspect, │ │ generator, init,   │
│               │ │ migration_  │ │ check-arch         │
│               │ │ file        │ │                    │
└───────┬───────┘ └──────┬──────┘ └────────┬───────────┘
        │                │                 │
        ▼                ▼                 ▼
┌─────────────────────────────────────────────────────────────┐
│                        rok-data-core                        │  ← sqlx 0.9, PostgreSQL
│        (the only place sqlx is depended on directly)        │    only
└───────┬─────────────────────────────────────────────────────-┘
        ▲
        │
┌───────┴───────────────┐      ┌───────────────────────────┐
│      rok-data-macros   │      │          rok-data          │  ← facade applications
│ derives + #[rok::test]  │ ←── │  depend on; re-exports     │    depend on only this
└────────────────────────┘      └───────────────────────────┘
```

(Also in the workspace: `sqlgen-db`, the generated-code example under
`examples/sqlgen/`, which exercises rok-data-codegen end to end, and
`examples/api`, the vertical slice of the pipeline — a schema, the
generated data layer and the axum API features beside it, and the staged
migration pairs — which the repository's own gates compile.)

The CLI's command surface has two generations: the front door (`rok init`,
`rok gen`) and the legacy commands (`rok generate`, `rok sqlc …`). The legacy
ones keep working exactly as they do now — `rok sqlc …` forwards its tokens
verbatim to the engine — but they are superseded by the front door, and
their removal is deferred to 1.0 (ADR 0012).

## Crate responsibilities

| Crate             | Responsibility                                                               | Depends on                          |
| ----------------- | ----------------------------------------------------------------------------- | ----------------------------------- |
| `rok-data-core`   | sqlx runtime: `Db`, builders, error, metrics, migrations, introspection        | sqlx 0.9 (postgres), tokio          |
| `rok-data-macros` | proc macros: `Model`/`FromRow`/`DbEnum`/`DbNewtype` derives, `#[rok::test]`    | proc-macro2, quote, syn             |
| `rok-data`        | facade: re-exports core + macros; the single dependency of applications        | core, macros                        |
| `rok-data-codegen`| model-crate generator (legacy `rok generate`): reads `.sql`, emits a cargo crate; stages migration pairs (`rok gen --migration`) | core (types), toml                  |
| `rok-data-sqlc`   | SQLx resource-module generator (legacy `rok sqlc generate|init|check-arch`); the API features have an actix-web and an axum backend   | own pg parser, clap  |
| `rok-cli`         | the `rok` binary: argument routing, output formats, connection handling        | codegen, core, sqlc, clap, tokio    |

## Dependency rules (enforced by review, and the reason the shape holds)

1. **Runtime crates never depend on `rok-cli`.** The CLI is a leaf; every
   capability it offers lives in a library (`rok_data_codegen::generate`,
   `rok_data_core::migration_file`, `rok_data_sqlc::cli::run_from`,
   `rok_data_sqlc::init` behind `rok init`). A future GUI, REST daemon or
   test harness would reuse exactly these.
2. **Only `rok-data-core` depends on sqlx.** Facade, codegen and the SQLc
   engine consume core's types; `rok-cli` takes its `Db` handle through
   `rok_data::sqlx` re-exports so the sqlx version stays pinned in one
   place.
3. **The CLI's sqlc subtree is the engine's own clap grammar.**
   The legacy `rok sqlc generate|init|check-arch` delegates straight to
   `rok_data_sqlc::cli::run_from`, so the engine's help text, aliases and
   exit codes remain the ones its tests assert. Nothing about the grammar
   is duplicated in rok-cli.
4. **No binary targets except `rok`.** The engine's old `main.rs` is gone;
   the model-crate generator's binary is replaced by
   `cargo run -p rok-data-codegen --example regenerate` (that example is
   how the workspace regenerates its own code).

## Layers of the migration system

- `rok-data-core::migration` — the migration engine: journal management,

  apply/revert with checksum-drift and edited-state detection, runs
  against the migrations loaded by a *source*.

- `rok-data-core::migration_file` — the file-source: loads
  `NNNN_name.up.sql`/`.down.sql` pairs (and accepts rok-fluent's
  single-file `-- up`/`-- down` sections), computes status rows
  (version/name/state/checksum/drift), journal existence, and next
  version hints. The CLI JSON contract is these `StatusRow`s.

- `rok-data-core::introspect` — information_schema/pg_catalog reader
  (tables, columns incl. PK detection, indexes, foreign keys) used by
  `rok db inspect` and `rok schema dump`.

## Generated-code contract

Both generators emit files whose first line carries a recognizable
marker (`@generated by rok-data-sqlc…` for the SQLc engine). Staleness
detection and `init --template` skipping recognize the header of the
consolidated engine **and** the legacy `rok-sqlc` marker, so existing
projects keep working unchanged until their next regeneration
(ADR 0005).
