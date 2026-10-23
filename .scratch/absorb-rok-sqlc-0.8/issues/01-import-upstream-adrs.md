# 01: Import upstream ADRs and record the absorption

**What to build:** Every decision the ported code relies on is readable in this workspace without number collisions. Upstream rok-sqlc engine ADRs 0017–0020 become engine ADRs 0019–0022 (0017 validation rules → 0019, 0018 auth as template features → 0020, 0019 axum → 0021, 0020 two-factor/API keys/files → 0022), each with a "Formerly rok-sqlc ADR NNNN" line. A new workspace ADR records the absorption, the freeze of rok-sqlc at 0.8.0, and the rule for numbering imported ADRs.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Four imported engine ADRs, upstream text kept, terms translated (`rok sqlc`, `rok.toml [generate]`)
- [ ] Where an imported ADR conflicts with a workspace decision (axum structure, engine ADR 0018 model shape), a "Superseded in this workspace by…" note
- [ ] Workspace ADR: absorption, freeze, imported-ADR numbering rule
- [ ] Spec: `docs/specs/absorb-rok-sqlc-0.8.md`
