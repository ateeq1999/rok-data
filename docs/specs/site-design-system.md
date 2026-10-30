# Spec: the docs site takes the web template's design system

The documentation site (`site/`, TanStack Start, deployed on Vercel) drops its
own cyan/teal "rok brand" theme and takes the design system of the monorepo
template's web App, so the site and the apps rok generates look like one
product. The source of truth is
`crates/rok-templates/templates/monorepo/apps/web/src/styles.css` and its
`components.json` (shadcn style `base-lyra`, base color `stone`).

Tickets: `.scratch/site-design-system/issues/`.

## Decisions

1. **The tokens are copied, never retyped.** The site's `styles.css` carries the
   template's `:root` block, `.dark` block, `@theme inline` block and base
   layer verbatim. A test fails when the site's copy and the template's file
   differ, so a change to the template's palette reaches the site in the same
   pull request.
2. **The site's own variables go.** `--sea-ink`, `--sea-ink-soft`, `--chip-bg`,
   `--chip-line`, `--line`, `--lagoon`, `--kicker`, `--surface*`, `--hero-*`,
   the `--rok-*` brand colors and the rest of the old theme are replaced by the
   shadcn tokens (`foreground`, `muted-foreground`, `border`, `card`, `primary`,
   ...) through Tailwind's classes (`text-muted-foreground`, `bg-card`,
   `border-border`). No page keeps a raw color.
3. **Fonts are bundled, not fetched.** Outfit (text) and Oxanium (headings)
   come from `@fontsource-variable`, at the template's versions, as the template
   loads them; code stays in a monospace face, also self-hosted. The Google
   Fonts import goes.
4. **The template's shape.** Square corners and the compact control sizes of
   `base-lyra`. Where the site needs a button, badge, card or separator, it
   uses the template's own `src/components/ui/` component, copied with the
   dependencies it needs at the template's versions.
5. **Dark mode keeps the site's toggle.** `ThemeToggle` already sets a `.dark`
   class on `<html>`, which the template's `.dark` block keys on; the
   `data-theme` attribute and the light/dark/auto choice stay.

Out of scope: the site's content and routes; changing the web template.

## Done means

- `pnpm build` and `pnpm test` pass in `site/`, and the token test is one of
  the tests.
- Every page, served from the build, renders in the new design in light and
  dark: no old variable is referenced anywhere in `site/src`
  (`grep -rE 'var\(--(sea|chip|lagoon|kicker|rok-|surface|hero|line|foam|sand)'`
  finds nothing).
- The `rok-data-docs-site` skill's guidance still matches the site (its JSX
  traps and the markup it tells a sync to follow).
