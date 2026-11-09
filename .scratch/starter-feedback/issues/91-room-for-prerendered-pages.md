# 91: Room for prerendered pages in the web App

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** the Monorepo web App's `vite.config.ts` ships:

- a `PRERENDERED: string[] = []` list, with the comment "add a marketing page by listing its path";
- `prerender: { enabled: PRERENDERED.length > 0, autoStaticPathsDiscovery: false, pages: PRERENDERED.map((path) => ({ path })) }`, so signed-in routes are never prerendered;
- the SPA shell masked at a path no page uses (`spa.maskPath`);
- a check that throws at config load when `PRERENDERED` holds the shell's path, naming both.

The web guide gains a short "Prerendered pages" section.

Karibu Pass prerendered its landing page at `/`, which silently replaced `_shell.html` and would have broken every other route on the static host (ticket 27, its ADR-0007).

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Listing `/about` builds a prerendered `/about` and keeps `_shell.html` (build test)
- [ ] Listing the shell's path fails the build, naming it
- [ ] With the list empty, the build is the SPA it is today
