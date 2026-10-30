---
name: rok-data-docs-site
description: >
  Bring the documentation site (`site/`, TanStack Start, deployed on
  Vercel) up to date with the repository. Use when: syncing the site
  after commits landed on main, adding a site page for a new crate,
  command or guide, or checking what the site is missing.
---

The site is a **mirror**: every claim on it comes from a source in this
repository. Its pages are hand-written TSX under `site/src/routes/`.

| Site page | Its sources |
| --- | --- |
| `routes/crates/<crate>.tsx` (one per published crate) | `crates/<crate>/README.md`, its `docs/`, CHANGELOG entries naming it |
| `routes/guide/installation.tsx` | `README.md`'s install section, `install.sh`/`install.ps1` |
| `routes/guide/getting-started.tsx` | `docs/getting-started.md` |
| `routes/agents.tsx` | `.agents/skills/*/SKILL.md` descriptions |
| `routes/index.tsx`, `components/Header.tsx` | the crate list and navigation: every page above |

A crate with no page yet gets one, linked from `index.tsx`.

## Steps

1. **Find the range.** `site/.synced-commit` holds the last commit the site
   reflects. The range is that commit to `main`'s tip:
   `git log --oneline <synced>..main` and `git diff --stat <synced>..main`.
   Done when you have the list of commits.
2. **List what changed for a reader.** From the range's CHANGELOG lines,
   READMEs, `docs/`, `docs/cli-reference.md`, crate guides and skills,
   write down each user-visible change and the site page it belongs on.
   Internal refactors, tests and CI need no page. Done when every commit is
   either mapped to a page or named as needing none.
3. **Edit the pages.** Follow the existing pages' markup and classes; take
   wording from the source, shortened, never invented. A version, flag,
   command or setting on the site must appear in its source. Done when each
   item from step 2 is on its page.
4. **Check the site.** In `site/`: `pnpm install` (once), `pnpm build` and
   `pnpm test`. Done when both pass.
5. **Move the marker.** Write the range's last commit (`main`'s tip at
   step 1) to `site/.synced-commit`.
6. **Commit** on the current branch: `site: sync to <short-sha>`, with the
   step 2 list in the body. Leave pushing and merging to the reviewer.
