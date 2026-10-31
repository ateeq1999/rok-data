# 01: the template's tokens and fonts, kept in sync by a test

**Blocked by:** none
**Status:** done

Spec: `docs/specs/site-design-system.md`, decisions 1, 3 and 5.

- Replace the old theme in `site/src/styles.css` with the template's token
  blocks, copied verbatim from
  `crates/rok-templates/templates/monorepo/apps/web/src/styles.css`: `:root`,
  `.dark`, `@theme inline` and the `@layer base` rules. Keep the site's own
  Tailwind setup (`@plugin "@tailwindcss/typography"`); add what the template's
  tokens need (`tw-animate-css`, `shadcn/tailwind.css`) only if the site uses
  it.
- Fonts: `@fontsource-variable/outfit` and `@fontsource-variable/oxanium` at the
  template's versions; headings use `--font-heading`. Code keeps a monospace
  face, self-hosted through `@fontsource`. Remove the Google Fonts import.
- A Vitest test (`site/src/styles.test.ts` or similar) reads both files and
  fails, naming the block, when the site's copy of any of the four blocks
  differs from the template's. Point `pnpm test` at it (the site has no other
  tests).

Done when `pnpm build` and `pnpm test` pass, the token test passes, and
changing one token in the template's file makes it fail (try it, then revert).
