# 15: The Dockerfiles' migrate stage uses the pinned `rok` release

Spec: `docs/specs/starter-feedback.md`

**What to build:** the templates' Dockerfiles (actix-api, axum-api, the Monorepo's API App) build their migrate stage with `cargo install rok-data-cli --locked --version 0.4.0` and copy its `rok`, which builds from source and is three minors old. Download the same pinned `rok` release CI uses (ticket 01), checksum verified, at the same `ROK_VERSION`.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] No template Dockerfile runs `cargo install rok-data-cli`
- [ ] The migrate stage's `rok` is the pinned release, checksum verified
- [ ] Ticket 01's test also covers the Dockerfiles' version
