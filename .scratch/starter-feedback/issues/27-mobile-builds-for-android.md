# 27: The mobile App builds for Android

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** the Monorepo mobile App drops `flutter_windowmanager` (v1 plugin API, no Gradle namespace: `flutter build apk` has never worked). `MainActivity.kt` sets `FLAG_SECURE` over a method channel, and `secure_screen.dart` calls it. The mobile gate runs `flutter build apk --debug` in CI, and skips it locally without an Android SDK, with a printed note.

**Blocked by:** grpc-api-template/08 (the gRPC Template shipped)

**Status:** ready-for-agent

- [ ] `flutter build apk --debug` passes in the mobile gate in CI
- [ ] Secure screens still block screenshots (method channel tested where it can be)
- [ ] `docs/security.md` updated
