# 59: A query's parameter shape doesn't flip

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** a query's `-- rok:params struct` or `-- rok:params positional` fixes the shape of its generated function, so adding a parameter never turns positional arguments into a `…Params` struct. Each generated function's doc comment states the default rule (up to three parameters positional, four or more a struct). The default is unchanged.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] `-- rok:params struct` on a two-parameter query generates a `…Params` struct; `positional` on a five-parameter one keeps arguments
- [ ] The doc comment names the rule
- [ ] Existing generated signatures unchanged (fixtures)
