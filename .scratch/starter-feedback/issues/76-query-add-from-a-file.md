# 76: `query add` reads its SQL from a file or stdin

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** `rok sqlc query add NAME SQL` takes its SQL argument the way `-j` takes a payload:
- **`@path`:** reads the SQL from that file.
- **`-`:** reads it from stdin.
- **Anything else:** inline SQL, as today.

Scripted edits then need no shell quoting on any OS (Karibu lost two rounds to heredocs and SQL quotes on Windows). The help and the engine README show `@path` for anything with quotes.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] `query add Name @q.sql` appends the file's SQL and generates (test)
- [x] `query add Name -` reads stdin (test)
- [x] A missing `@path` fails naming it
