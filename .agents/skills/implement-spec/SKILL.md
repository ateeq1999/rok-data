---
name: implement-spec
description: Implement a **spec's task graph** to green gates, ticket by ticket, with **implementer subagents**. Trigger branches the name or the situation — usually invoked as `/implement-spec`: a spec (this repo's unified pipeline plan, an ADR, or a file the user names) is to be implemented; "the remaining Phase X" is asked for; or an interrupted implementation resumes.
argument-hint: "A spec to implement, or a phase to resume"
---

Read the spec, derive its **task graph**, and drive it — ticket by
ticket, **frontier** first — until the whole spec is implemented and
every gate is green. Specs in this repo usually live as a phase
section of `docs/unified-pipeline-plan.md`, an ADR under
`docs/decisions/`, or a markdown file the user names. Tickets may
already be written down (the plan's bullets, numbered or checkboxed);
derive them from the prose when not, and state the derived graph back
to the user before the first implementer runs.

## Context pointers

Communicate to and from subagents sparsely, primarily through
**context pointers**: the spec, the ADRs, `GLOSSARY.md`, prior
commits, and per-ticket briefs. Do not duplicate what a pointer
carries.

**Exploration work** (codebase reading, doc research) belongs to an
exploration subagent that writes notes OUTSIDE the repo (under the
system temp dir), so implementer subagents inherit pointers, not
reading lists.

## Execution modes

Pick by tree state before the first implementer, and say which:

- **In-tree** (default; required when the working tree carries
  uncommitted work — this repo's usual state): one implementer
  subagent at a time, working in the real tree. No worktrees: a
  worktree cannot see uncommitted changes and would throw them away.
- **Worktree** (only when the tree is clean to the user's last
  commit): each implementer runs in its own worktree, merged back
  with a merger subagent that resolves conflicts and re-runs the
  gates.

## Each ticket

1. Name the implementer subagent's scope: the ticket, its
   dependencies, and pointers. It works like this repo does — the
   `tdd` skill when a ticket is best built test-first, and until every
   gate is green: `cargo fmt --all`, `cargo clippy --workspace
   --all-targets -- -D warnings`, `cargo test --workspace
   --all-targets`.
2. Snapshots and expected outputs move only through their sanctioned
   mechanism (`ROK_DATA_SQLC_UPDATE_EXPECTED = 1`), never by hand.
   Never push; commits stay with the user unless they have explicitly
   asked.
3. On green, state the ticket done in one line each: what changed, how
   the gates read. Anything that cannot pass the gates goes back into
   a repair ticket, not into the user as a "mostly done".

## Closing

1. When the graph is exhausted, run the `code-review` skill on the
   spec's change set and fix what it finds in one more implementer
   subagent.
2. Report the spec implemented: what landed, what was deferred (with
   where it was recorded — a backlog bullet or an ADR), and the gate
   evidence. Leave commits to the user.
