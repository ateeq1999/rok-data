# 07: The rule catalogue moves into the runtime

**What to build:** The engine's generated rule implementations (`email`, `url`, `mobile` with its locales, `postal_code`, `uuid`, `ip`, `regex`, `alpha`, `alpha_numeric`, `in`, `not_in`, `confirmed`, `same_as`, number/date/array bounds) become runtime functions usable from `#[rok(validate(…))]` on models and inputs; generated code calls them instead of carrying copies.

**Blocked by:** 02

**Status:** ready-for-agent

- [ ] Each rule in the derive grammar, with its code and default message unchanged
- [ ] Engine generation switched to the runtime functions; snapshots refreshed by the sanctioned run only
- [ ] Template gates unchanged on the wire
