# 05: The remaining Modules

Spec: `docs/specs/starter-feedback.md` · workspace ADR 0033

**What to build:** `media`, `audit-log`, `admin`, `totp` and `theming` become Modules of the Monorepo template (and of the single-app Templates where they exist), each confined to its files and declared wiring points, with the needs between them declared.

**Blocked by:** 04

**Status:** blocked

- [ ] Each Module can be left out alone; the all-out project passes the gates
- [ ] Template tests no longer assume a left-out Module's routes
