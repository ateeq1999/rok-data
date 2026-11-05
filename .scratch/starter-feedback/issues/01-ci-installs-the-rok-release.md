# 01: Template CI installs the pinned `rok` release

Spec: `docs/specs/starter-feedback.md`

**What to build:** in rok-templates, every template's CI workflow downloads the `rok` release tarball for `x86_64-unknown-linux-musl` from rok-v2's GitHub release (pinned by a `ROK_VERSION` variable), checks it against its `.sha256`, and puts `rok` on the PATH. This replaces `cargo install rok-data-cli --locked`, which builds from source and does not install `rok`. The checks the workflow runs stay as they are. A rok-templates test checks that `ROK_VERSION` is a rok release linking the rok-data minor in `rok-data.pin`.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] actix-api, axum-api (grpc-api once rok-templates#4 lands) and the Monorepo's API App install `rok` from the release, checksum verified
- [x] `ROK_VERSION` agrees with `rok-data.pin` (unit test)
- [ ] The workflow's install step runs green once in CI (checked in a Debian container; rok-templates' own CI never runs template workflows)
- [x] Template READMEs say how CI gets `rok`
