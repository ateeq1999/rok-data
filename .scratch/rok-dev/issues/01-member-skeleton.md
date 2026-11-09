# 01: The rok-dev member and its words in rok

Spec: `docs/specs/rok-dev.md` · workspace ADR 0034

**What to build:** the `ateeq1999/rok-dev` repo (the maintainer creates it on GitHub) with a `rok-dev-cli` crate and a `rok-dev` binary, its own CI and release workflow modelled on rok-tools, a `GLOSSARY.md` defining **Runnable**, and `AGENTS.md`. rok-v2 links `rok-dev-cli` and routes `dev`, `run`, `doctor` and `port` to it; each command prints "not built yet" until its ticket lands. The meta-repo's map lists the member.

**Blocked by:** None (the GitHub repo is the maintainer's to create)

**Status:** done

- [x] `rok dev --help` lists the four words through rok-v2
- [x] CI runs fmt, clippy and tests on Linux, macOS and Windows
