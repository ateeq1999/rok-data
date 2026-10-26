# 01: Import upstream ADRs and record the absorption

**What to build:** Every decision the ported code relies on is readable in this workspace without number collisions. Upstream rok-sqlc engine ADRs 0017–0020 become engine ADRs 0019–0022 (0017 validation rules → 0019, 0018 auth as template features → 0020, 0019 axum → 0021, 0020 two-factor/API keys/files → 0022), each with a "Formerly rok-sqlc ADR NNNN" line. A new workspace ADR records the absorption, the freeze of rok-sqlc at 0.8.0, and the rule for numbering imported ADRs.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] Four imported engine ADRs, upstream text kept, terms translated (`rok sqlc`, `rok.toml [generate]`)
- [x] Where an imported ADR conflicts with a workspace decision (axum structure, engine ADR 0018 model shape), a "Superseded in this workspace by…" note
- [x] Workspace ADR: absorption, freeze, imported-ADR numbering rule
- [x] Spec: `docs/specs/absorb-rok-sqlc-0.8.md`

**How it shipped:**
- **The imported ADRs:** engine ADRs 0019–0022 keep upstream's text. Their cross-references are renumbered (upstream 0018 → 0020, 0019 → 0021), and each heads with a status line and "Formerly rok-sqlc ADR NNNN".
- **Workspace notes:** each ADR carries an "In this workspace" note.
- **The axum conflict:** engine ADR 0021 carries a "Superseded in this workspace" note. A1's spelling (`actix_web`, `-w/--api-framework`), A3 (generated handlers take `Extension`, with one `router(pool)`, not `State`/`FromRef`) and A7 (the `tests/axum-app` fixture gated by rok-cli) are the workspace's. Engine ADR 0019 notes that the checks take engine ADR 0018's shape.
- **The new workspace ADR:** workspace ADR 0016 records the absorption, the freeze and the numbering rule.
- **AGENTS.md:** the ADR counts now read 0016 and 0022.
- **The spec:** `docs/specs/absorb-rok-sqlc-0.8.md` already existed.
