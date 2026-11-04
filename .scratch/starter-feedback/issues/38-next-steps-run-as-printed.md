# 38: "Next steps" commands run as printed

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** `rok gen`'s and `rok init`'s "Next steps" print commands that work from the directory they were run in. With ticket 24, `rok migrate up` needs no `--dir`, so it's printed bare. Any path printed is relative to the current directory.

**Blocked by:** 24

**Status:** blocked

- [ ] Each printed command runs as printed, from the root and from inside the App (test)
