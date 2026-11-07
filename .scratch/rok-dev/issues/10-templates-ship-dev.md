# 10: The templates ship their `[dev]` tables

Spec: `docs/specs/rok-dev.md` · workspace ADR 0034

**What to build:** in rok-templates, each template's `rok.toml` (the single-app templates gain one) ships `[dev]` tables: db (compose, `port:`), api (`cargo run -p <api>`, `http:/health/live`, `migrate = "apply"`), and in the Monorepo web (`bun run dev`, `port:3001`) and mobile (`flutter run`, `device`). READMEs show `rok dev`, `rok doctor` and `rok port`.

**Blocked by:** 04, 07

**Status:** blocked

- [ ] A fresh Monorepo scaffold starts with `rok dev` (manual check, recorded)
- [ ] READMEs updated
