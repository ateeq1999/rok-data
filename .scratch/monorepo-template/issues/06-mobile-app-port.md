# 06: Mobile App imported and ported — apps/mobile on the monorepo contract

**What to build:** The Flutter starter (`ateeq1999/Nestjs-Better-Auth-Flutter-Starter`) is imported as the monorepo's `mobile` App and its data layer is ported to the monorepo backend's contract **through the generated Dart client** (05) — its NestJS/Better-Auth client code is gone, and the app talks to `apps/api`. Surfaces the backend doesn't serve yet (organizations, magic link, admin, delete account) are hidden, not half-dead; push notifications stay out as a written recipe decision (FCM, ADR 0023). Firebase configuration ships as a placeholder (it is tied to one Firebase project) plus the recipe to make it real. Init writes `apps/mobile`; the app runs against the local api (sign in, core screens); it is gated like the rest: `flutter analyze`, `flutter test` from `fixture_compile/`, skip-with-note without the toolchain, CI installs it.

**Blocked by:** 05 (generated Dart Client — the port's data layer).

**Status:** ready-for-agent

- [ ] `apps/mobile` is vendored (single source of truth — upstream archived later, ticket 09) and init writes it like the other Apps: files-if-absent across the whole monorepo, embedding with no stray platform build output or secrets (real `google-services.json`/Firebase values can never ship; the placeholder does)
- [ ] The data layer is generated-client-only: HTTP calls, models and error handling come from `clients/dart`; hand-written NestJS-era api code is removed; auth/session logic cards through the generated client against the api's actual endpoints
- [ ] Unservable features are hidden: navigation/entries for organizations, magic link, admin and delete-account point nowhere or are absent, each with a note of the backend gap, so the app remains browsable and honest
- [ ] Firebase placeholder + recipe; screenshots/tests updated to the local contract; `flutter analyze` and `flutter test` pass on the scaffolded app (live locally against `apps/api`)
- [ ] Gate: fixture_compile test runs the flutter gates with skip-with-note when the toolchain is absent; CI installs the toolchain; format/clippy and all Rust suites stay green
