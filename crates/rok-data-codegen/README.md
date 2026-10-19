# rok-data-codegen

`rok` turns `.sql` files into a Rust crate for [rok-data](https://crates.io/crates/rok-data):

- models and enums from `CREATE TABLE` / `CREATE TYPE`;
- typed async functions from sqlc-style annotated queries;
- migrations computed from schema changes.

```sh
cargo install rok-data-codegen
rok generate                        # db/*.sql -> db-gen/
rok generate --migration add_slug   # after changing the schema
rok check                           # CI: fail when db-gen/ is stale
rok migrate status                  # which migrations DATABASE_URL has applied
rok migrate up                      # apply pending migrations (a deploy step)
rok migrate down --steps 1          # revert the latest one
```

```sql
-- db/user.sql
CREATE TABLE users (
    id    BIGSERIAL PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    name  TEXT
);

-- name: find_by_email :one
SELECT * FROM users WHERE email = $1;
```

becomes `db-gen/src/user.rs`, with this usage:

```rust
db_gen::up(&db).await?;                                       // migrations
let user: Option<db_gen::user::User> = db_gen::user::find_by_email(&db, "ann@example.com").await?;
let named = db_gen::user::User::filter(db_gen::user::User::NAME.is_not_null()).all(&db).await?;
```

Query types are checked by PostgreSQL. Set `DATABASE_URL` when queries change: a temporary
database is created and dropped, and the results are cached in `db-gen/queries.json`.

See the [design and full reference](https://github.com/ateeq1999/rok-data/blob/main/docs/v4.md)
and the [example project](https://github.com/ateeq1999/rok-data/tree/main/examples/sqlgen).

License: MIT OR Apache-2.0.
