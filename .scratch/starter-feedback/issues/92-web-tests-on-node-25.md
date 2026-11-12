# 92: The web App's tests run on Node 25

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** the Monorepo web App's `vitest.config.ts` sets `test.execArgv: ["--no-experimental-webstorage"]`, with a comment saying why: Node 25+ defines a `localStorage` global, `undefined` without `--localstorage-file`, that hides jsdom's. The rok-templates CI's web job also runs on the current Node release, beside the pinned one.

Every Karibu Pass test using storage failed on Node 25 with "Cannot read properties of undefined (reading 'getItem')" (ticket 27).

**Blocked by:** None (can start immediately)

**Status:** done (rok-templates PR #16)

- [x] The web App's tests pass on the current Node release in CI
- [x] A storage test (`session.test.ts`) passes on Node 25
