# 16: A Client can be emitted as plain sources

Spec: `docs/specs/starter-feedback.md` · workspace ADR 0027 (amended 2026-11-03)

**What to build:** in the engine, a `[clients]` entry can be a table:

```toml
ts   = { dir = "apps/web/src/lib/api", layout = "sources" }
dart = { dir = "apps/mobile/lib/app/data/api", layout = "sources" }
rust = "apps/api/client"
```

**Layouts:**
- **`package`:** the plain-string form means `layout = "package"`, today's output.
- **`sources`:** writes only the source files, flat in `dir`:
  - TypeScript: `index.ts`, `runtime.ts`, `types.ts`, `calls.ts`, `queries.ts`.
  - Dart: `api.dart` (the export file), `runtime.dart`, `models.dart`, `client.dart`.
  - No README, `pubspec.yaml`, `analysis_options.yaml` or `src/` level.
- **Rust:** `layout = "sources"` for the Rust Client is a configuration error.

`rok gen` owns only the `@generated` files in the directory; `rok gen check` and stale-file cleanup follow the existing marker rules.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Both forms parse; an unknown `layout`, or `sources` for Rust, fails naming the entry
- [ ] Emission tests for each layout of the TypeScript and Dart Clients
- [ ] A hand-written file in a `sources` directory survives `rok gen`
- [ ] Engine docs and CLI reference describe the table form
