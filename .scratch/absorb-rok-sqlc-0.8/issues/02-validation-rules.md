# 02: Validation rules (rok-sqlc 0.5)

**What to build:** A developer declares string rules and transforms per table and column under `rok.toml [generate]` (`[api.rules]`, VineJS names), and the generated API enforces them, collecting every field failure of a write (`FieldFailures`) into one `422`. A bad `regex` rule fails `rok sqlc generate`, not a request.

**Blocked by:** dx-first 03 (generated verbs on the model — validation is expressed on that shape)

**Status:** ready-for-agent

- [ ] `[api.rules]` grammar in the engine CLI/config; `docs/cli-reference.md` updated in the same change
- [ ] Generated `shared/validation` with `FieldFailures`
- [ ] `regex-syntax` check at generation time
- [ ] Upstream `api_rules` suite ported into rok-cli's binary-driven tests
- [ ] Snapshots refreshed via the sanctioned run; fixture compile gates green
