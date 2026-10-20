# rok-api-example

The vertical slice of [rok-data](https://github.com/ateeq1999/rok-data):
one schema, one `rok gen`, and the whole application around it.

- `schema/` — the truth (two plain `.sql` files) and the history (the
  staged migration pairs);
- `src/db/**` — the generated data layer: the ORM chains over `authors`,
  `books` and `loans`;
- `src/features/**` — the generated API features: the axum routes,
  handlers, bodies and OpenAPI schemas;
- `src/{app,config,error,lib,main,openapi}.rs`, `src/shared/auth.rs` and
  the features' `service.rs` — the hand-written half, from the axum
  template.

It is a crate of its own (its own `[workspace]` table, listed in the root
`Cargo.toml`'s `exclude`), so it builds on its own and never joins the
workspace it lives in.

## How it was made

The front door, in three commands:

```console
$ rok init my-api --template axum      # the hand-written half
$ cd my-api
$ rok gen                               # the data layer and the features
```

This example is that project: the axum template's hand-written half,
copied and adapted, with a schema of its own — `schema/library.sql` and
`schema/loans.sql` where the template has `migrations/`, and its own
`rok.toml`.

`rok gen` reads the schema dir and writes `src/db/` and `src/features/`;
`rok gen --migration NAME` writes the difference between the schema and the
last staged migration as a `NNNN_name.up.sql` / `.down.sql` pair beside it.
`rok gen check` is the CI mode: it writes nothing and fails on drift.

The two truth files are the schema as it is now:

- `schema/library.sql` — `authors` and `books`;
- `schema/loans.sql` — `loans`, the table the books are lent from.

The history beside them is what the schema dir recorded on the way:

- `schema/0001_init.up.sql` / `.down.sql` — the two tables of the first
  file;
- `schema/0002_add_loans.up.sql` / `.down.sql` — the third table and the
  foreign key that wires it to the books.

The truth is what the generator reads; the pairs are the history. That is
why the generator never applies a change twice: it reads `library.sql` and
`loans.sql`, not the pairs.

## Running it

You need PostgreSQL and Rust 1.94 or newer.

1. Apply the staged migrations, in order, to a database of your own:

   ```console
   $ psql "$DATABASE_URL" -f schema/0001_init.up.sql
   $ psql "$DATABASE_URL" -f schema/0002_add_loans.up.sql
   ```

   The runtime's own loader (`rok migrate up --dir`) reads a directory of
   migration pairs and nothing else, and the schema dir holds the truth
   beside them; that gap is recorded in
   [`docs/unified-pipeline-plan.md`](../../docs/unified-pipeline-plan.md).

2. Set `DATABASE_URL` and `JWT_SECRET` (`.env.example` lists every
   setting; `src/config.rs` reads them from the environment).
3. `cargo run`. The API is under `/api/v1`, health at `/health/live` and
   `/health/ready`, the OpenAPI document at `/api-docs/openapi.json` and
   Swagger UI at `/docs/`.

Every route but health needs `Authorization: Bearer <token>`: an HS256 JWT
signed with `JWT_SECRET`, with `sub` (the caller's id), `role` (`user` or
`admin`) and `exp`. `src/shared/auth.rs` verifies it. Issuing tokens (sign
in) is not part of the example; see the template it comes from.

## The API

Three features, one per table:

| Route | What it does |
| --- | --- |
| `GET /api/v1/authors?q=&sort=&order=&page=&per_page=` | a page of authors; `q` matches `name` and `email` |
| `POST /api/v1/authors` | creates an author |
| `GET /api/v1/authors/{id}` | one author |
| `PATCH /api/v1/authors/{id}` | changes an author |
| `DELETE /api/v1/authors/{id}` | deletes an author |
| `GET /api/v1/authors/by-email/{value}` | the author with that email |
| `GET /api/v1/authors/{id}/books` | the author's books |
| `GET /api/v1/books` | a page of books |
| `POST /api/v1/books` | creates a book of an author |
| `GET /api/v1/books/{id}`, `PATCH`, `DELETE` | one book |
| `GET /api/v1/books/by-isbn/{value}` | the book with that ISBN |
| `GET /api/v1/books/{id}/loans` | the book's loans |
| `GET /api/v1/loans`, `POST` | a page of loans, lending a book |
| `GET /api/v1/loans/{id}`, `PATCH /api/v1/loans/{id}` | one loan, returning it |

`rok.toml` decides the rest: `[api.skip]` leaves the loans' `remove`
endpoint out (a loan is returned, not deleted), and `[api.search]` and
`[api.sort]` give the lists their `q` and their sort keys.

## Checks

```console
$ cargo fmt --check
$ cargo clippy --all-targets -- -D warnings
$ DATABASE_URL=postgres://... cargo test   # each test makes and drops its own schema
$ rok gen check                             # the generated code matches the schema
```

Without `DATABASE_URL`, the API tests print a note and pass; the health
test needs no database. The lints in `Cargo.toml` deny `unwrap`, `expect`,
`panic`, `todo` and leftover debugging, and `src/lib.rs` denies a public
item without documentation.

The repository's own gate compiles this example
(`crates/rok-cli/tests/fixture_compile/example_api.rs`): it checks the
drift, stages the migration pairs again in a copy and compares them, and
runs the checks above.

## What is generated, and what is yours

- **Generated** (first line `@generated`; `rok gen` rewrites them, never
  edit them): `src/db/` (the data layer), `src/shared/{health,pagination,
  response,tenant,validation}.rs`, and every file of `src/features/` but
  `mod.rs` and each feature's `service.rs`.
- **Yours** (first line `Scaffolded once`, or the example's own header):
  `rok.toml`, `schema/`, `src/{lib,main,app,config,error,openapi}.rs`,
  `src/shared/{mod,auth}.rs`, `src/features/mod.rs`, each feature's
  `service.rs` (business rules go there), `tests/`, and this README.

Change the schema, run `rok gen`, and the features follow.
