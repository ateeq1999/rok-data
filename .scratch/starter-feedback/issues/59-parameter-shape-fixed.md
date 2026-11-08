# 59: A query's parameter shape doesn't flip

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:**
- **Per query:** `-- rok:params struct` or `-- rok:params positional` fixes the shape of its generated function, so adding a parameter never turns positional arguments into a `…Params` struct.
- **Per project:** `rok-data-sqlc.toml`'s `[queries] params` sets the default.
  - `"auto"`: today's rule, positional up to three parameters and a struct from four. It stays the default.
  - `"struct"`: every query takes its `…Params` struct, so a growing query never changes its call shape.
  - `"positional"`: every query keeps positional arguments.

  The per-query annotation wins.
- **Doc comment:** each generated function's doc comment states the rule that applied.

Karibu Pass's call sites broke when `SetLocation` (ticket 09) and `OpenWeekends` (ticket 25) grew a fourth parameter.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] `-- rok:params struct` on a two-parameter query generates a `…Params` struct; `positional` on a five-parameter one keeps arguments
- [x] `[queries] params = "struct"` makes every query take its struct; the annotation overrides it
- [x] The doc comment names the rule; existing generated signatures unchanged under `"auto"` (fixtures)
