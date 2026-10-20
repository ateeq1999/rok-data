---
name: loop-me
description: Run a **grilling** session that turns the user's recurring **loops** (weekly rituals, morning routines, repeated chores of work or life) into implementable **workflow specs** stored under `.agents/loop-me/`. Trigger branches the user explicitly asks for it — usually by invoking `/loop-me`; fires alone when they say they want a workflow designed or sharpened.
argument-hint: "A workflow to design, or nothing to go find one"
---

Grill the user — load the `grilling` skill and follow its discipline
(relentless, a round of questions at a time, a recommended answer on
each) — with one iron lens: the only output is **workflow specs**, and
each is done when an implementer agent could build it without asking a
single question. Create, edit, and delete specs as grilling resolves
things; consult the state first, update it after every session.

## The loop lens

A **loop** is a recurring pattern in the user's life: career, week,
morning, a repeated activity. Picturing a life as loops within loops
shows how predictable its activities are, which is what makes them
worth delegating. Use the lens to propose loops the user hasn't
noticed, not just the ones they bring.

A **workflow** is one loop's spec, made real: you *run* a workflow on a
loop. Workflows are the source of truth.

## Vocabulary

Shared language, reached for only when a workflow calls for it. Never
a checklist: mandate no AI, no checkpoint, no schedule unless the
grilling shows that workflow needs it.

- **Trigger**: what fires a run — an **event** (an email arrives, a
  repo pushes) or a **schedule** (every morning). Prefer events; they
  describe more and poll less.
- **Checkpoint**: a human-decision point in the loop. Some workflows
  have none and run autonomously; some use no AI at all.
- **Push right**: defer the checkpoint as far as it goes. Do maximal
  work first, ask once, late, with everything prepared.
- **Brief**: what a checkpoint presents — a tight, decision-ready
  summary (what was produced, why, link down to the asset), never raw
  output. The user reads a brief, not a draft.

When a spec is implementable, offer to hand it to the `implement-spec`
skill and treat the result as its run.

## The workspace

- `.agents/loop-me/workflows/*.md` — one spec per workflow.
- `.agents/loop-me/NOTES.md` — the user's world: tools, channels,
  events, and their own names for all of them; sharpen fuzzy words
  into canonical ones as they surface.

When `NOTES.md` is empty or thin, interview the user about their world
before specifying anything. Each session may skim the current
workflows to find the gaps worth grilling about.
