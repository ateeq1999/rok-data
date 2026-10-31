# Issue tracker: Local Markdown

Issues and specs for this repo live as markdown files in the repository, not
on GitHub Issues.

## Conventions

- One feature per directory: `.scratch/<feature-slug>/`.
- The spec is `docs/specs/<feature-slug>.md` (most features since 0.5.0) or
  `.scratch/<feature-slug>/spec.md`; an ADR under `docs/decisions/` can stand in
  for a spec (the monorepo template's tickets follow workspace ADR 0027). A
  ticket names its spec in its first lines.
- Implementation issues are one file per ticket at
  `.scratch/<feature-slug>/issues/<NN>-<slug>.md`, numbered from `01`, never a
  single combined tickets file.
- Near the top of each issue: a `**Blocked by:**` line (`none`, or the ticket
  numbers it waits on, `NN, NN`) and a `**Status:**` line. The status is one of
  the triage roles in `triage-labels.md`, or `blocked` (waiting on its blockers)
  or `done` (finished and committed). A ticket is ready when every ticket it
  lists is `done`.
- Comments and conversation history append to the bottom of the file under a
  `## Comments` heading.

## When a skill says "publish to the issue tracker"

Create a new file under `.scratch/<feature-slug>/` (creating the directory if
needed), following the conventions above.

## When a skill says "fetch the relevant ticket"

Read the file at the referenced path. The user will normally pass the path or
the ticket number directly.

## When a skill closes work

Set the ticket's `**Status:**` to `done` in the commit that finishes it.

## Wayfinding operations

Used by `/wayfinder`. The **map** is a file with one **child** file per ticket.

- **Map**: `.scratch/<effort>/map.md` (the Notes / Decisions-so-far / Fog body).
- **Child ticket**: `.scratch/<effort>/issues/NN-<slug>.md`, numbered from `01`, with the question in the body. A `Type:` line records the ticket type (`research`/`prototype`/`grilling`/`task`); a `Status:` line records `claimed`/`resolved`.
- **Blocking**: a `Blocked by: NN, NN` line near the top. A ticket is unblocked when every file it lists is `resolved`.
- **Frontier**: scan `.scratch/<effort>/issues/` for files that are open, unblocked, and unclaimed; first by number wins.
- **Claim**: set `Status: claimed` and save before any work.
- **Resolve**: append the answer under an `## Answer` heading, set `Status: resolved`, then append a context pointer (gist + link) to the map's Decisions-so-far in `map.md`.
