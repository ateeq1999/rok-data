# The rok-data documentation site

The site in this folder is rok-data's documentation, deployed on Vercel. It
mirrors the repository: every page is hand-written TSX under `src/routes/`
whose claims come from a source in the repository (a crate's README, a page
under `docs/`, the CHANGELOG). The `rok-data-docs-site` agent skill
(`.agents/skills/rok-data-docs-site/SKILL.md`) says which source feeds which
page and how to bring the site up to date; `.synced-commit` holds the last
commit the site reflects.

It is a [TanStack Start](https://tanstack.com/start) app with file routes,
rendered on the server and built to `.output/`, styled with Tailwind CSS v4
and the typography plugin.

## Run, build and test

```bash
pnpm install
pnpm dev        # http://localhost:3000
pnpm build      # writes .output/
pnpm test       # Vitest: the design-token test
```

To look at the production build, serve it and open
http://localhost:3917:

```bash
PORT=3917 node .output/server/index.mjs
```

## Design

The site uses the design system of the web App in the monorepo template
(`crates/rok-templates/templates/monorepo/apps/web`, shadcn style `base-lyra`,
base color `stone`), so the documentation and the apps rok generates look
like one product (`docs/specs/site-design-system.md`):

- **Tokens.** `src/styles.css` carries the template's `:root`, `.dark`,
  `@theme inline` and `@layer base` blocks, copied verbatim from the
  template's `src/styles.css`. `src/styles.test.ts` fails, naming the block,
  when the copy and the template differ: change the template, then copy the
  block. Pages color through the tokens' Tailwind classes
  (`text-muted-foreground`, `bg-card`, `border-border`, `text-primary`), never
  a raw color; the `prose` pages read the same tokens.
- **Components.** `src/components/ui/` holds the template's own `button`,
  `badge` and `card`, copied unchanged. Copy another one from the template the
  same way, with its dependencies at the template's versions.
- **Fonts.** Outfit (text) and Oxanium (headings) from `@fontsource-variable`,
  as the template loads them, and JetBrains Mono for code; all bundled, none
  fetched from a font service.
- **Dark mode.** `ThemeToggle` cycles light, dark and auto, and sets the
  `.dark` class on `<html>` that the template's `.dark` block keys on.
