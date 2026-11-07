# 08: `rok doctor`

Spec: `docs/specs/rok-dev.md` · workspace ADR 0034

**What to build:** `rok doctor` checks Docker (running), cargo, bun, Flutter, the Android SDK with `adb`/`emulator`, the configured AVD, `xcrun` on macOS, and the `[dev]` ports, plus the known traps: the Kotlin incremental "different roots" failure when the project and the pub cache are on different drives (`--fix` writes `kotlin.incremental=false` to the App's `gradle.properties`), and the Gradle 8.14 / AGP 8.11 / Kotlin 2.2 deprecations. Each check prints ok, warn or fail with its fix; any fail exits non-zero; `--json` for CI.

**Blocked by:** 02

**Status:** blocked

- [ ] The drive check and `--fix` (unit tests)
- [ ] Each check through the faked runner
