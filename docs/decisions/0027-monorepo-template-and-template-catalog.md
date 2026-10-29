# ADR 0027: a monorepo template, and rok-templates owns the catalog

Date: 2026-10-29 · Status: accepted (amends ADR 0022)

## Decision

- A **Monorepo template** joins the single-app Templates: one backend,
  a web app and a mobile app sharing one API contract.

  ```text
  apps/api/        axum-api, reused file for file (template_drift covers it)
  apps/web/        the TanStack Start starter
  apps/mobile/     the Flutter starter
  clients/ts, clients/dart, clients/rust
                   generated from apps/api's openapi.json
  rok.toml         names the apps; `rok gen` regenerates the api and the clients
  ```

  Plain bun inside `apps/web`; no JS monorepo tool.

- **One backend, one contract.** The backend is `axum-api` only (actix
  stays a single-app Template). The TanStack starter already calls a
  subset of `axum-api`'s spec. The Flutter starter was written against a
  NestJS / Better-Auth backend whose code is not in its repository; its
  data layer is ported to `axum-api`'s contract through the generated
  Dart client. Its features the backend does not serve yet
  (organizations, magic link, admin, delete account) are hidden until
  `axum-api` grows them; push notifications pick a vendor (FCM), so they
  are a written recipe under ADR 0023.

- **rok-templates owns the catalog** (amends ADR 0022): template names,
  their manifests, the Apps of a monorepo, and resolving a name to the
  files a project gets. The engine's `init` asks it for that file plan
  and keeps the write policy — copy when absent, skip `@generated` files
  — so the marker rules (ADR 0005) stay the engine's, the reason ADR
  0022 rejected moving the copy logic. The `rok sqlc init` grammar stays
  in the engine's `cli.rs` (invariant 3).

- **rok-templates is the only source.** The two starters' GitHub
  repositories (`ateeq1999/tanstack-starter-axum`,
  `ateeq1999/Nestjs-Better-Auth-Flutter-Starter`) are imported, then
  archived with a pointer here; a second place to edit would drift where
  `template_drift` cannot see it.

- **rok emits the Clients itself**, from `openapi.json` — not from the
  engine's model, because the hand-written auth, 2FA, passkey and QR
  routes exist only in the spec — covering the OpenAPI 3.1 subset utoipa
  emits and no more. The TS client gives typed calls plus TanStack
  Query `queryOptions`/`mutationOptions` factories and replaces the web
  starter's `lib/http.ts` and `features/*/api.ts`; its error type keeps
  their `ApiError`/`FieldIssue` shape. The Rust client is a `reqwest`
  crate with its own types (callers do not compile the server); a root
  Cargo workspace holds `apps/api` and `clients/rust`, and the API's
  integration tests call through it, so client and server are proved
  to agree.

- **Init renames every app**: `--org` (default `com.example`) and the
  project name set the crate and package names, the Flutter
  `applicationId` and iOS bundle id, the deep-link scheme and the
  display name. Firebase configuration is tied to one Firebase project,
  so it ships as a placeholder plus a recipe.

- **Non-Rust apps are gated like the Rust ones.** Dependencies are pinned
  to exact versions (no `"latest"`). `apps/web` is checked with
  `bun install --frozen-lockfile`, typecheck and tests, `apps/mobile`
  with `flutter analyze` and `flutter test`, from
  `crates/rok-cli/tests/fixture_compile/`; each skips with a printed note
  when its toolchain is absent, and a CI job installs both.

## Considered Options

- **A NestJS backend app** beside, or instead of, `axum-api`: it would be
  written from scratch and double the contract the clients follow.
- **Re-vendoring from the upstream repositories**: two sources of truth.
- **Moving `init` wholesale into rok-templates**: breaks invariant 3.
- **Existing client generators** (openapi-typescript or hey-api,
  openapi-generator for Dart, progenitor): less to maintain, but three
  tools' call sites, none of them shaped by us.

## Consequences

- The `rok` binary embeds the web and mobile apps, Flutter's platform
  folders included (about 1.5 MB together).
- rok-templates gains code, not only data: the crate's "files and nothing
  else" doc and ADR 0022's "data only" no longer hold.
