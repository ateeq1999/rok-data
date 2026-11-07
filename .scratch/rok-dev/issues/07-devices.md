# 07: Devices, and the API address they need

Spec: `docs/specs/rok-dev.md` · workspace ADR 0034

**What to build:** `--device` (default from `[dev.<mobile>] device`): `android:<avd>` reuses a booted emulator or boots `emulator -avd <avd> -no-snapshot-load` and waits for `sys.boot_completed`, clearing a hung instance and its `multiinstance.lock` from the directory the AVD's `.ini` `path` names; `ios[:<name>]` uses `xcrun simctl boot`; `lan` / `usb` a physical device via `flutter devices`. A mobile Runnable depending on the API gets `--dart-define=API_URL=<origin>`: `10.0.2.2` (emulator), `127.0.0.1` (simulator), the LAN IP (phone), with the API's port.

**Blocked by:** 04

**Status:** blocked

- [ ] The `.ini` → lock path and the origin per target are unit-tested
- [ ] Device commands go through a runner trait faked in tests
