# 03: every page in the new design

**Blocked by:** 02
**Status:** ready-for-agent

Spec: decision 2 and Done means.

Every route under `site/src/routes/` (index, about, agents, crates/*,
guide/* including guide/templates/*) moves off the old variables onto the
tokens. The `prose` typography follows the tokens too (body, headings, links,
code, tables), in light and dark. Badges, chips and cards use the copied ui
components.

Done when the spec's grep finds nothing in `site/src`, `pnpm build` and
`pnpm test` pass, and every page, served from the build
(`PORT=3917 node .output/server/index.mjs`), shows its own content in the new
design.
