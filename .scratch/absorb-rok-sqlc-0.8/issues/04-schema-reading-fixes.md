# 04: Schema-reading fixes (rok-sqlc 0.8)

**What to build:** pg_dump-shaped schemas generate correctly: `ALTER TABLE … DROP CONSTRAINT [IF EXISTS]` is followed; dropping an unknown constraint warns instead of failing the table; `CHECK (col = ANY (ARRAY[...]))` is read as a value list; `RENAME CONSTRAINT` checks both names; `IS NULL` matches in any case.

**Blocked by:** dx-first 01

**Status:** ready-for-agent

- [ ] Each case covered in the engine's migration-alters library suite
- [ ] Warning text printed, never silent
