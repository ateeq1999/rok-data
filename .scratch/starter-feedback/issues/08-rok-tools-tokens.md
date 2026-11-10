# 08: `rok tools tokens`

Spec: `docs/specs/starter-feedback.md`

**What to build:** in rok-tools, a `tokens` command that reads a `tokens.json` and writes the web CSS tokens, the Dart tokens file and the desktop Rust tokens, each marked generated; `--check` fails when any output is stale. Reachable as `rok tools tokens`.

**Blocked by:** None (can start immediately)

**Status:** done (rok-tools #4; the release tag is the maintainer's)

- [x] Unit tests over a sample `tokens.json` for each output
- [x] `--check` fails on a hand-edited output
- [ ] A rok-tools release carrying it (the tag is the maintainer's)
