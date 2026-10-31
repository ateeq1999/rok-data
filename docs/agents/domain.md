# Domain Docs

How the engineering skills should consume this repo's domain documentation when
exploring the codebase. rok-data is **multi-context**.

## Before exploring, read these

- **`GLOSSARY-MAP.md`** at the repo root. It points at one `GLOSSARY.md` per
  context; read each one relevant to the topic:
  - `GLOSSARY.md` (the workspace: the `rok` CLI, the pipeline, product terms)
  - `crates/rok-data-sqlc/GLOSSARY.md` (the generator engine)
  - `crates/rok-templates/GLOSSARY.md` (the starter projects: Template,
    Capability, Shared file)
- **ADRs** that touch the area you're about to work in. This repo keeps them in
  two series, not in `docs/adr/`:
  - `docs/decisions/` (workspace ADRs: cross-crate decisions)
  - `crates/rok-data-sqlc/docs/adr/` (engine ADRs: the engine's own decisions)

  The numbers collide by design; cite "workspace ADR NNNN" or "engine ADR NNNN"
  wherever the number alone is ambiguous (see AGENTS.md, Docs discipline).

If any of these files don't exist, **proceed silently**. Don't flag their
absence; don't suggest creating them upfront. The `/domain-modeling` skill
creates them lazily when terms or decisions actually get resolved.

## File structure

```
/
├── GLOSSARY-MAP.md
├── GLOSSARY.md                       ← the workspace context
├── docs/decisions/                   ← workspace ADRs
└── crates/
    ├── rok-data-sqlc/
    │   ├── GLOSSARY.md
    │   └── docs/adr/                 ← engine ADRs
    └── rok-templates/
        └── GLOSSARY.md
```

## Use the glossary's vocabulary

When your output names a domain concept (in an issue title, a refactor
proposal, a hypothesis, a test name), use the term as defined in the relevant
`GLOSSARY.md`. Don't drift to synonyms the glossary explicitly avoids.

If the concept you need isn't in a glossary yet, that's a signal: either you're
inventing language the project doesn't use (reconsider) or there's a real gap
(note it for `/domain-modeling`).

## Flag ADR conflicts

If your output contradicts an existing ADR, surface it explicitly rather than
silently overriding:

> _Contradicts workspace ADR 0027 (the monorepo template), but worth reopening because…_
