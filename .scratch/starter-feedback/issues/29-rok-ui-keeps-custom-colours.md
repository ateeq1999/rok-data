# 29: rok-ui keeps a custom theme's colours

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** in rok-ui, a `Theme` holds a `ThemePair` (its light and dark colours). `toggle_mode`, `sync_with_system_appearance` and `change_preset` rebuild from the pair, so a custom theme keeps its colours across a mode switch. A preset is a built-in pair, and `Theme::custom(name, pair)` makes one. Released as rok-ui 0.10.0 (the tag is the maintainer's). The Monorepo desktop App then drops any `set_mode` workaround of its own.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] A custom theme survives `toggle_mode` and `sync_with_system_appearance` (tests)
- [ ] Presets behave as before
- [ ] rok-ui changelog and docs updated
