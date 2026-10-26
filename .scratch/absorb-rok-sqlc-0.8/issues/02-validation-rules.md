# 02: Validation rules (rok-sqlc 0.5)

**What to build:** A developer declares string rules and transforms per table and column under `rok.toml [generate]` (`[api.rules]`, VineJS names), and the generated API enforces them, collecting every field failure of a write (`FieldFailures`) into one `422`. A bad `regex` rule fails `rok sqlc generate`, not a request.

**Blocked by:** dx-first 03 (generated verbs on the model — validation is expressed on that shape)

**Status:** done

- [x] `[api.rules]` grammar in the engine CLI/config; `docs/cli-reference.md` updated in the same change
- [x] Generated `shared/validation` with `FieldFailures`
- [x] `regex-syntax` check at generation time
- [x] Upstream `api_rules` suite ported into rok-cli's binary-driven tests
- [x] Snapshots refreshed via the sanctioned run; fixture compile gates green

**How it shipped:**
- **Copied from upstream v0.5.0 (with its review fixes):** the rule model and parser (`src/api_rules.rs` and its tests), and the generator's `rules`, `rule_functions`, `rule_snippets` and `rule_tables`. They have no workspace counterpart, so they came over as files, with the tool name and crate path translated.
- **Wiring, re-done on the workspace's code:** `ApiConfig::rules` and `check_rules`, the TOML/JSON `RuleSpec`, `Feature::rules`/`rules_of`, the create and update bodies in `dto.rs` (the same places as upstream), `push_signature_where`, and the `regex` hint in `db/mod.rs`.
- **`FieldFailures`:** appended to `shared/validation.rs` for both frameworks. Upstream had only actix's template there; here it is written with full `validator::` paths, so neither template changes.
- **Tests:**
  - the engine's `tests/api_rules.rs` (library, 8);
  - rok-cli's `tests/cli/api_rules.rs` (binary, 2);
  - the api-app's `0005_profiles.sql` with `tests/rules.rs` (DB-backed, 3), configured in `fixture_compile/api_app.rs`.
- **Regeneration:** snapshots, `milestone-1.txt`, the templates' and fixtures' `validation.rs`, `examples/api` and the engine's `docs/examples/api` were all regenerated through the sanctioned runs or `rok gen`.
- **Templates:** they gain no `[api.rules]` yet. Upstream's template change belongs with the template tickets.
