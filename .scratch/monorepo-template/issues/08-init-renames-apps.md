# 08: Init renames every App

**What to build:** Init names every App after the project, per ADR 0027: `--org` (default `com.example`) joins the init front doors, and with the project name it drives the renames — the api App's crate name (the renaming init already does), the web App's package name, the mobile App's Dart/Flutter package name, the Flutter `applicationId` and iOS bundle id (`<org>.<project>`), the deep-link scheme, and the display name. Renames happen at scaffold time; scaffolding without `--org` still works exactly as today with the default org. Manual-rename footnotes vanish (or the template's own docs shrink to the exceptions, like Firebase).

**Blocked by:** 06 (mobile App ported — every App's rename surface exists).

**Status:** ready-for-agent

- [ ] `--org` exists on both front doors (`rok init`, engine's init grammar — tokens forwarded verbatim), defaulting to `com.example`, validated to look like a reverse-domain name
- [ ] Scaffold with a project name + org renames every App in one pass: api crate/package, web package, mobile package + `applicationId` + iOS bundle id + deep-link scheme + display name; the rename rules are unit-tested per App (segment-matching conventions for the code files, whole-identifier renames for manifests)
- [ ] The rename respects everything init already respects: files-if-absent, marker rules, generated files untouched; `--org` with a name that would break an identifier is rejected cleanly
- [ ] The non-Rust gates stay green with a renamed project (analyze/tests/typecheck run on the renamed scaffold), proving the renames didn't break any App
- [ ] `docs/cli-reference.md` updated in the same change (the init contract: `--org`, which names derive); format/clippy/test suites green
