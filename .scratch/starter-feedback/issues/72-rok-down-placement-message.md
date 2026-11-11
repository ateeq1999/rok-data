# 72: Say where `-- rok:down` goes

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** the rule stays: each statement rok can't undo by itself carries its own `-- rok:down` block immediately after it. The message changes from "rok can't undo this statement by itself; follow it with `-- rok:down` …" to name the statement (its first line) and say "add `-- rok:down …` immediately after this statement". The migrations guide shows an example with two triggers, each followed by its own block.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] The message names the statement and says "immediately after" (test)
- [x] The guide's example
