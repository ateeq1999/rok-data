# 22: A changed function is replaced, not dropped

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** in migration staging:
- **Same signature:** a `CREATE FUNCTION` whose text changed but whose name and argument types didn't is staged as `CREATE OR REPLACE FUNCTION`, and a changed trigger as `CREATE OR REPLACE TRIGGER` (PostgreSQL 14+), instead of its `-- rok:down` followed by its new text.
- **New signature:** a changed signature is refused with an error naming the function and, where the schema shows them, the triggers that use it.

**Blocked by:** 21

**Status:** done

- [x] Changing a trigger function's body stages one `CREATE OR REPLACE FUNCTION` and applies on a database where three triggers use it (`crates/rok-data-codegen/src/ddl/replace/tests.rs`; `crates/rok-cli/tests/gen/replaced_function.rs` applies and reverts it with `rok migrate up`/`down`)
- [x] Changing its argument types is refused, naming the triggers
- [x] Changelog updated (and `docs/cli-reference.md`)
