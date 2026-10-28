# 07: The rule catalogue moves into the runtime

**What to build:** The engine's generated rule implementations (`email`, `url`, `mobile` with its locales, `postal_code`, `uuid`, `ip`, `regex`, `alpha`, `alpha_numeric`, `in`, `not_in`, `confirmed`, `same_as`, number/date/array bounds) become runtime functions usable from `#[rok(validate(…))]` on models and inputs; generated code calls them instead of carrying copies.

**Blocked by:** 02

**Status:** done (2026-10-27)

- [x] Each rule in the derive grammar, with its code and default message unchanged
- [x] Engine generation switched to the runtime functions; snapshots refreshed by the sanctioned run only
- [x] Template gates unchanged on the wire

**Progress (2026-10-27) — split by decision:**
- [x] **07a:** the regex-free rules (`url`, `uuid`, `ulid`, `ip`, `alpha`, `alpha_numeric`, `ascii`, `one_of`/`not_one_of` (the engine's `in`/`notIn`; `in` is a Rust keyword), `starts_with`, `ends_with`, `hex_code`, `iban`, `jwt`, `credit_card`, `coordinates`) in the derive grammar; their checks extracted verbatim from the engine's snippets into `rok-data-core::validate::catalogue`; codes and messages are the engine's (the wire contract: codes stay camelCase where the engine's are, e.g. `alphaNumeric`, overriding the spec's "snake_case" for these). `normalize_email` follows the engine's semantics.
- [x] **07b:** `mobile` (locales), `postal_code` (countries) and `regex` (a `regex` dependency behind a runtime feature), length-in-characters rules (`min_length`/`max_length`/`fixed_length` as the engine's), and the engine's generation switched to call the runtime catalogue (sanctioned snapshot refresh).
  The engine keeps generating the regex-backed checks itself (patterns embedded, the app's own `regex` dependency) rather than requiring the runtime's `regex` feature in every generated manifest; the pattern tables are shared by test, not by code.
