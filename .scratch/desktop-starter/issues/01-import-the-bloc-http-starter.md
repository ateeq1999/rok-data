# 01: import rok-ui's bloc-http starter as apps/desktop

**Blocked by:** none
**Status:** done

Spec: `docs/specs/desktop-starter.md`, What ships (the import) and Gates.

Generate `cargo rok-ui new --template bloc-http` at a pinned rok-ui release,
import it into `crates/rok-templates/templates/monorepo/apps/desktop`, and align
it mechanically (package name, root workspace member, the API address). Add the
App to the catalog (`MONOREPO_APPS`), the `fixture_compile` gate, and the
`desktop` CI job. Embedding excludes `target/`.

Done when a fresh `rok init --template monorepo` builds and tests
`apps/desktop` through the fixture, CI has the job, and the package check
still passes.
