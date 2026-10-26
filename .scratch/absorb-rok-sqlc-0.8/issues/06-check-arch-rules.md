# 06: `check-arch` rules (rok-sqlc 0.6 + 0.7)

**What to build:** `rok sqlc check-arch` flags a feature that queries another feature's tables (`cross-feature-query`) and a feature that imports the other framework's crate.

**Blocked by:** dx-first 01

**Status:** done

- [x] Both rules in the engine; cases in rok-cli's `check_arch` binary-driven suite
- [x] `docs/cli-reference.md` lists the rules

**How it shipped:**
- **`cross-feature-query`:** came with ticket 05.
- **The other framework's crate:** this ticket adds `Rule::OtherFramework`. The crate the config doesn't name is a `rule-1` violation everywhere but the data layer. It is built on the workspace's `Rule1` shape, not upstream's `FRAMEWORK_*` constants.
- **A workspace decision superseded:** "rule 1 polices the configured framework alone". The two tests asserting it, the CLI `rule_1_watches_the_framework_the_config_sets` and the unit test, are rewritten to expect the violation.
- **Tests:** upstream's `tests/cli/check_arch_axum.rs` is ported to rok-cli. Its messages name the framework as the workspace does (`actix_web`), and the config file is `rok-data-sqlc.toml`.
- **Moved to ticket 07:** upstream's `init --framework axum` case. The workspace's `init` has no framework flag for the skeleton yet.
- **CLI reference:** `docs/cli-reference.md` gains a table of every `check-arch` rule.
