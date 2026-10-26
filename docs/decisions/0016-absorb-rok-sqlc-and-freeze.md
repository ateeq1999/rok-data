# ADR 0016: absorb rok-sqlc 0.5–0.8 into the engine, freeze rok-sqlc

Date: 2026-10-26 · Status: accepted

## Context

The `rok sqlc` engine (`rok-data-sqlc`) was imported from the standalone
`rok-sqlc` repository at v0.4.0, as a plain copy with no shared history
(workspace ADR 0003). Since then both copies kept moving. Upstream shipped
0.5 (validation rules), 0.6 (auth as template features), 0.7 (actix
hardening, axum) and 0.8 (two-factor, API keys, uploads, the storage port).
This workspace reshaped the engine on its own: `rok.toml` (workspace ADR
0011), the model-centric generated shape (engine ADR 0018), inferred
conventions, named scopes, its own axum support.

Two copies of one engine mean that every fix lands twice, or lands once
while the other copy rots. The engine ADR series already collides:
upstream's 0017–0020 and this workspace's engine 0017–0018 share numbers
but record different decisions.

## Decision

- **One engine.** Every user-visible capability of rok-sqlc 0.5.0 through
  0.8.0 is brought into `rok-data-sqlc`, one release at a time, in order
  (`docs/specs/absorb-rok-sqlc-0.8.md`, tickets
  `.scratch/absorb-rok-sqlc-0.8`).
- **A behavioural port, not a merge.** Upstream commits are replayed as
  behaviour onto this workspace's code, translated into its terms: the
  `rok sqlc` front door, `rok.toml`, the model-centric generated shape,
  and versioned migration names. An upstream file may be copied over a
  workspace file only where the workspace copy is still byte-identical to
  upstream v0.4.0.
- **The workspace wins a conflict.** Where the two sides decided the same
  question differently, the workspace's decision stands unless the spec
  says otherwise. The imported ADR records the conflict in a
  "Superseded in this workspace" note. Engine ADR 0021 (axum) is the
  main case.
- **The freeze.** When the 0.8 port lands, rok-sqlc is frozen at 0.8.0.
  Its README says so and points to `rok sqlc`, and its final CHANGELOG
  entry does the same. It takes no new features and needs no new release.
  The repository is not deleted or archived.
- **The numbering rule for imported ADRs.** An imported ADR keeps its
  upstream text and takes the **next free number** of the series it joins,
  in upstream order. Its header carries a "Formerly <repo> ADR NNNN" line,
  and its cross-references are rewritten to the new numbers. No existing
  ADR is ever renumbered. For this absorption, upstream engine ADRs 0017,
  0018, 0019 and 0020 become engine ADRs 0019 (validation rules), 0020
  (auth as template features), 0021 (axum) and 0022 (two-factor, API keys
  and files). A later absorption, if any, follows the same rule.

## Considered Options

- **Keep both engines and port fixes both ways.** Rejected: that doubles
  every fix, the copies already differ in 153 of 193 shared files, and
  there would be nothing to stop them drifting further.
- **Merge the git histories.** Rejected: the copies share no history, and
  the workspace's restructuring would turn a merge into a rewrite anyway.
  A behavioural port can be reviewed release by release.
- **Freeze without porting.** Rejected: users of `rok sqlc init` would lose
  two-factor, API keys, uploads, the 0.7 security fixes and the 0.5
  validation rules that the standalone tool already ships.
- **Renumber the existing engine ADRs to make room.** Rejected: code,
  docs and commit messages already cite them by number ("engine ADR
  0016"), and renumbering would break every cite.

## Consequences

- The engine ADR series reaches 0022. The two series still number
  independently, so a bare number stays ambiguous: cite "engine ADR NNNN"
  or "workspace ADR NNNN" (AGENTS.md).
- The engine's CHANGELOG names each absorbed upstream release, so the
  question "is fix X in the workspace?" has a written answer.
- New template dependencies (`aes-gcm`, `totp-rs`, `object_store`, …)
  stay in the templates' own manifests. The engine crate gains only
  `regex-syntax`, and sqlx stays `rok-data-core`'s alone (AGENTS.md).
- Until the absorption tickets land, the imported ADRs describe behaviour
  that is not here yet. Their status line says so.
