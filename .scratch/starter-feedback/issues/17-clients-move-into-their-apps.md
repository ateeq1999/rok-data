# 17: The Monorepo's Clients move into their Apps

Spec: `docs/specs/starter-feedback.md` · workspace ADR 0027 (amended 2026-11-03)

**What to build:** the Monorepo template drops the root `clients/` directory.

**Where each Client goes:**
- **Web:** the TypeScript Client is emitted into `apps/web/src/lib/api/` (`layout = "sources"`). `src/lib/client.ts` imports `./api`, and the `@client` alias and the TanStack Query path workaround go.
- **Mobile:** the Dart Client is emitted into `apps/mobile/lib/app/data/api/` (`layout = "sources"`). The repositories import `../api/api.dart`, the mobile `pubspec.yaml` gains `http` pinned exactly, and `analysis_options.yaml` excludes `lib/app/data/api/**`.
- **Rust:** the Client moves to `apps/api/client/`, with the root workspace members, the API's dev-dependency and the desktop dependency updated. The desktop App reaches it only through a hand-written `src/data/providers/api.rs`.

**Docs:** the Monorepo README and each App's architecture notes state the rule: a Client lives where its App keeps its data sources, and the App reaches it only through a hand-written adapter. The migration guide shows the move as an optional manual step for existing projects.

**Blocked by:** 16 (released in rok-data), grpc-api-template/08 (the gRPC Template shipped)

**Status:** blocked

- [ ] No root `clients/`; `rok gen` and `rok gen check` are clean on a fresh scaffold
- [ ] The web, mobile, desktop and API gates pass
- [ ] The `--apps` work drops an App's Client and its `[clients]` entry with it
- [ ] Docs and migration guide updated
