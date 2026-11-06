# Spec — The starter, fixed from a project's first build

Status: ready-for-agent · Implements workspace ADR 0033 · Source: Karibu Pass's build notes ("What the rok starter cost us", 2026-11-03, commits `fa369ae`, `bf9b676`, `32651f6`)
Interview outcome in one line: fix the rok side of what scaffolding a Monorepo project cost — a CI gate that installs `rok` in seconds, `-w` for a Template's API App, Modules left out with `--without`, one source for design tokens, kits inside their Apps, and a local setup that works on the first run — and leave the project's own product work (phone sign-in, its tickets) to it.

## Problem Statement

A team scaffolded the Monorepo template with `rok init` and built three Apps on top of it. Before writing product code they spent time on the template itself:

- **The API framework was fixed:** the Monorepo template's API App is axum only, so a team that had chosen actix ported the OpenAPI example and two tests by hand, and has to re-check that port on every rok upgrade.
- **The CI gate was slow, and as shipped it doesn't work:** each template's CI runs `cargo install rok-data-cli --locked`, which builds from source on every run, then calls `rok`, a binary that package does not install.
- **Init flags were unclear:** `-w` can't be combined with `-t`, and the usage error doesn't say why; `-s` seeds a root `schema/` the Monorepo template never reads.
- **Parts they didn't need had to be removed by hand:** API keys, media library, audit log, admin users, TOTP and theming, across API, web and mobile and the template's own tests. The TOTP issuer named the crate (`karibupass-api`).
- **One design system existed in three copies:** the UI kits shipped as packages broke bun's `file:` dependency, and once moved into each App the tokens lived in TypeScript, Dart and Rust with nothing keeping them in step.
- **Template files weren't formatted to current tools,** so `dart format` rewrote 57 of them, and CRLF line endings broke scripted edits on Windows.
- **Local setup had gaps:**
  - The API doesn't read `.env`.
  - No dev account can sign in without setting `email_verified` in Postgres by hand.
  - Postgres's port 5432 is hard-coded.
  - `tsr generate` crashes.
  - The web tests need a Node flag on Node 26.
- **The mobile screens could not be checked against the design** without a running API, so the team wrote, then deleted, a render test of its own.

## Solution

**CI:** a scaffolded project's CI installs the pinned `rok` release binary in seconds and runs the same checks it runs today.

**Init:** `rok init kp -t monorepo -w actix_web` gives an actix API App, and `--without api-keys,media,audit-log,admin,totp,theming` leaves out Modules the project doesn't need, with the rest still compiling and passing its tests. `-s` seeds the schema directory `rok gen` reads.

**Design tokens:** one `tokens.json` generates the web, mobile and desktop tokens, and CI fails if they drift. Each UI kit lives in its App's own source.

**Templates:** they ship formatted, with LF line endings enforced.

**Local setup:**

- the API reads `.env` in debug builds
- a dev seed creates verified accounts
- Postgres's port is a setting
- the web App can regenerate its route tree on its own
- the web tests run on any Node

**Visual check:** one golden test checks a mobile screen against its design without an API.

## User Stories

1. As a developer starting a project, I want `rok init` to scaffold the Monorepo template with an actix API App, so that the framework my team chose comes generated and kept current by rok, not ported by hand.
2. As a developer, I want `-w` combined with a single-app template to explain that the template is one framework already and name the template to use instead, so that I don't decode a usage error.
3. As a developer, I want `rok init … --without api-keys,media` to leave those Modules out, so that I don't unwire them by hand across three Apps.
4. As a developer, I want a project with Modules left out to compile and pass its tests straight from `rok init`, so that leaving a Module out is safe.
5. As a developer, I want `rok init` to refuse to leave out a Module that another kept Module needs, naming both, so that I never get a project that doesn't build.
6. As a developer, I want a Blueprint to remember the API framework and the Modules left out, so that every project created from it starts the same.
7. As a developer, I want `-s schema.sql` to land in the schema directory `rok gen` reads, and the init summary to say where, so that my seed schema is used.
8. As a developer, I want my project's CI to install `rok` in seconds instead of building it, so that the drift check doesn't slow every push.
9. As a template maintainer, I want the pinned `rok` version in the template's CI checked against `rok-data.pin`, so that the two can't disagree.
10. As a developer, I want the TOTP issuer to be a setting that defaults to my project's name, so that authenticator apps show my product, not my crate.
11. As a designer-developer, I want one `tokens.json` to produce the web, mobile and desktop tokens, so that the three Apps can't drift apart.
12. As a developer, I want CI to fail when a generated tokens file is stale, so that a token change reaches every App.
13. As a developer, I want each UI kit in its App's own source, so that package-manager quirks like bun's `file:` copies can't break the build.
14. As a developer, I want template files formatted with the tools' pinned versions, so that my first format run doesn't rewrite files I never touched.
15. As a developer on Windows, I want the project to enforce LF line endings, so that scripted edits and byte-compared checks behave the same as on Linux.
16. As a developer, I want the API to read `.env` when I run it in debug, so that `cargo run` works without exporting variables.
17. As a developer, I want a dev seed that creates a verified admin and a verified ordinary user and prints their credentials, so that I can sign in on the first run.
18. As a developer, I want Postgres's host port to be a setting, so that an existing local Postgres doesn't collide with the project's.
19. As a developer, I want a script that regenerates the web route tree with the router's own CLI version, so that I don't need a full build to add a route.
20. As a developer, I want the web tests to run on the current Node without me adding flags, so that a Node upgrade doesn't break `bun run test`.
21. As a mobile developer, I want a golden test of a screen with the real fonts, so that I can check the screen against its design without a running API.

## Implementation Decisions

- **CI install** (every template's workflow):
  - **Install:** replace `cargo install rok-data-cli --locked` with downloading the rok release tarball for `x86_64-unknown-linux-musl` from rok-v2's GitHub release, checking it against its `.sha256`, and putting `rok` on the PATH.
  - **Pin:** a `ROK_VERSION` variable in the workflow pins the release.
  - **Check:** a rok-templates test checks that `ROK_VERSION` is a rok release linking the rok-data minor in `rok-data.pin`.
  - **Commands:** each template's checks stay as they are (`rok sqlc g --check` and `rok sqlc check-arch` for single-app templates, `rok gen check` for the Monorepo).
- **`-w` with a template:**
  - **Combining:** `--api-framework` combines with a Template that has an API App and picks that App's framework (`actix_web`, `axum`, `grpc`; default `axum`).
  - **Source:** the Monorepo's API App is drawn from the matching single-app Template, so the framework variants are not kept by hand a second time.
  - **Single-app templates:** for a single-app Template, `-w` is an error that names the Template to use instead.
- **Modules** (workspace ADR 0033):
  - **Catalog:** the rok-templates catalog declares each Module of a Template (its name, its files per App, its wiring points, the Modules it needs).
  - **Flag:** `--without <MODULE,…>` drops those files and wiring lines at init, and refuses to drop a Module a kept one needs.
  - **First Modules:** `api-keys`, `media`, `audit-log`, `admin`, `totp`, `theming`.
  - **Capabilities:** Capabilities are not Modules.
  - **Blueprint:** a Blueprint records the API framework and the Modules left out.
- **`-s` seeds** the schema directory the project's `rok.toml` names for the App `rok gen` generates from (in the Monorepo, the `api` App's), and the init summary prints the path. Single-app templates keep `schema/0001_init.sql`.
- **TOTP issuer:** a `TOTP_ISSUER` setting, defaulting to the project name without a trailing `-api`.
- **Design tokens:**
  - **Source:** the Monorepo template ships one `tokens.json`.
  - **Generator:** `rok tools tokens` (rok-tools) writes the web `styles.css` tokens, the mobile Dart tokens file and the desktop Rust tokens from it; `--check` fails when any is stale, and the template's CI runs it.
  - **Kits:** each UI kit moves into its App's own source; no kit ships as a package.
- **Formatting and line endings:** template files are formatted with the pinned `dart format` and Prettier versions, the gates fail on an unformatted template file, and every Template ships `.gitattributes` with `* text=auto eol=lf`.
- **Local setup:**
  - **`.env`:** the template APIs load `.env` with dotenvy in debug builds only.
  - **Dev seed:** a `seed` binary creates a verified admin and a verified ordinary user and prints their credentials; `REQUIRE_VERIFIED_EMAIL` stays `true` by default.
  - **Postgres port:** `compose.yaml` maps `"${POSTGRES_PORT:-5432}:5432"`, `.env.example` sets `POSTGRES_PORT` and uses it in `DATABASE_URL`, and the stale `actix_api` database name in the Monorepo API's `.env.example` is fixed.
- **Web App:**
  - **Route tree:** `@tanstack/router-cli` is pinned to the router's version and a `routes` script runs `tsr generate`; the web gate runs it and fails if `routeTree.gen.ts` changes.
  - **Tests on Node 26:** the `test` script sets `NODE_OPTIONS=--no-experimental-webstorage`, and the gates pin the Node version.
- **Mobile golden test:** the Monorepo's mobile App ships one golden test of its sign-in screen, with the OFL fonts bundled so it renders the same on every machine.
- **Clients live in their Apps** (workspace ADR 0027, amended 2026-11-03): a Client lives where its App keeps its data sources, and the App reaches it only through a hand-written adapter.
  - **Paths:** TypeScript in `apps/web/src/lib/api/`, Dart in `apps/mobile/lib/app/data/api/`, Rust as a crate at `apps/api/client/` (the API's tests prove it; the desktop App's `api` provider wraps it).
  - **Layout setting:** a `[clients]` entry can be a table with `dir` and `layout`; `layout = "sources"` emits only the source files, flat (TypeScript and Dart only), and the plain-string form keeps today's package layout, so existing projects are unaffected.

## Testing Decisions

- **Gates:** they test what a user's project gets, through `rok init` and the project's own checks, not template internals.
- **Modules:**
  - **Builds:** for every Template with Modules, the gates build and test the default and the all-out combination.
  - **Engine tests:** `--without` with an unknown Module, and with a Module a kept one needs, are usage errors with their own tests.
- **CI install:** the `ROK_VERSION` check is a rok-templates unit test. One gate run with the workflow's install step confirms the download and checksum work.
- **Tokens:** `rok tools tokens` has unit tests over a sample `tokens.json` (each output's content), and a `--check` test that fails on a hand-edited output.
- **Init:** `-w` with `-t monorepo`, `-w` with a single-app template, and `-s` with the Monorepo are init tests in rok-data, beside the existing ones.
- **Dev setup:** the seed binary is tested against the gates' database (it creates two accounts that can sign in). The route-tree and formatting checks are part of the web and mobile gates.

## Typed web data: zod in the TypeScript Client

- **Schemas:** the TypeScript Client emits a zod schema per component schema (`userResponseSchema`), with the type as its `z.infer`, and `oneOf` as `z.union` (engine ADR 0027, amended 2026-11-03).
- **Responses checked:** each call parses its response and throws a `ContractError` (the operation and zod's issue paths) on a mismatch, apart from the HTTP `ApiError`. Request bodies are sent unchecked; the server validates them.
- **The server's rules:** `minLength`/`maxLength`, `minimum`/`maximum`, `pattern` and `format` (`email`, `uuid`, `date-time`) become zod rules in the TypeScript Client only.
- **The web App:** `features/*/schemas.ts` keeps only the forms' own rules, built on the generated request schemas with `.extend`/`.pick`/`.refine`. Login's deliberate "no length hint", the TOTP and recovery-code formats, and the confirm fields stay hand-written; the response schemas it duplicated are deleted. `zod` is pinned exactly, and the web gate checks it against the zod major the Client's header names.

## Bugs found building on it

From Karibu Pass's notes on tickets 02 and 03 (commits `4f194dc`, `1ff04a9`, rok 0.7.0, rok-ui 0.9.1), checked against rok-data 0.8.0 and rok-ui 0.9.2:

- **Un-typeable queries fail by default:** `rok gen` ran the engine leniently, so a query whose result column it can't type (`expires_at <= now() AS expired`) was skipped with a stderr warning that scrolled past. `rok gen` is now strict by default (`--lenient` to skip), and the error keeps the `-- column: <name> <RustType>` hint.
- **Line endings don't change migrations:** migration staging (rok-data-codegen) compared statements byte for byte, so a CRLF schema file made an unchanged function and trigger look changed. Staging normalises line endings in the truth files and the snapshot, and rok-data-core's migration checksum is computed over LF-normalised text. The migration guide notes the one-time checksum change.
- **A changed function is replaced, not dropped:** a `CREATE FUNCTION` whose text changed but whose name and argument types didn't is staged as `CREATE OR REPLACE FUNCTION` (and a trigger as `CREATE OR REPLACE TRIGGER`). A change of signature is refused, naming the dependent objects. Full dependency tracking is out of scope.
- **Errors show their causes:** every CLI error prints its whole chain, so "failed to parse SQL schema" carries the file, the line and the parser's message.
- **One rule for finding the project:**
  - **Search:** every command (`rok gen`, `rok gen check`, `rok migrate *`, `rok schema *`, `rok db *`, `rok sqlc check-arch`) walks up from the current directory to the nearest `rok.toml`.
  - **Inside an App:** it acts on that App.
  - **At the root:** it acts on the App(s) `[apps]` names; `rok migrate up` takes the Apps' migrations directories.
  - **Report:** each command prints one line naming the config and directories it read.
  - **Item 4:** the notes' "reads the wrong schema dir inside the App" does not reproduce from the code; the report line will show it, and Karibu is asked for their App's `rok-data-sqlc.toml`.
- **Partial failure is reported:** when a `rok gen` step fails, it ends with the steps done, the step that failed, the steps skipped, and the command that finishes the rest. Nothing is rolled back.
- **`check-arch` says it passed:** `checked N features: no violations`.
- **The mobile App builds for Android:** `flutter_windowmanager` (v1 plugin API, no Gradle namespace) is replaced by a method channel that sets `FLAG_SECURE` from `MainActivity.kt`. The mobile gate runs `flutter build apk --debug`, skipping with a note without an Android SDK.
- **The actix contract test is a parity check:** the README says it is expected to grow `KNOWN` entries as a project changes its own schema. A `KNOWN_SCHEMAS` list marks a whole schema as the project's own, and the stale "Coming next" section is fixed.
- **rok-ui keeps custom colours:** a `Theme` holds a `ThemePair` (light and dark colours); `toggle_mode`, `sync_with_system_appearance` and `change_preset` rebuild from it, a preset is a built-in pair, and `Theme::custom(name, pair)` makes one (rok-ui 0.10.0).

From the notes on Karibu Pass's ticket 04:

- **`SCHEMA_SQL` is in dependency order:** the engine emits it with statements in the order the migration diff uses, so a forward reference across schema files no longer breaks the tests that load it. Its doc comment says the tests load it.
- **Casts keep nullability:** a cast keeps the nullability of what it wraps (`count(*)::bigint`, a `NOT NULL` column), and `coalesce(x, <literal>)` is not-null. The `-- column:` override is documented for the rest.
- **Constraint violations say which constraint:** the templates' `AppError::Conflict` becomes `Conflict { kind, constraint, message }` (`kind` is `Unique` or `ForeignKey`), still 409, so a feature maps a constraint to its own error by name.
- **Flows other features compose run on the caller's connection:** `invite_in`, `register_in` and the audit log's `record_in` take a connection; the existing methods open their transaction and call them. The template's `AGENTS.md` states the convention.
- **A browser smoke test:** the web App ships Playwright (pinned) with one flow (sign in with the seed's account, land on home, sign out). `bun run e2e` runs it, and the web gate runs it in CI with the API up.
- **The dev server's address is IPv4:** `server.host: "127.0.0.1"`, with the proxy target on `127.0.0.1:8080`.
- **The first admin:** the dev seed's `--promote <email>` (ticket 11).

From the notes on Karibu Pass's ticket 05:

- **A foreign key without `_id` names its relationship after both:** `offers.reviewed_by → users` gives `reviewed_by_user`, instead of a relationship named like the column that collides with the search filter's field (the data layer derives both for every table, excluded from the API or not).
- **`rok` reads the App's `.env`:** `rok migrate`, `rok db`, `rok schema` and `rok gen`'s database steps load `<app>/.env`; the environment wins, the config line says so, and `--no-env-file` opts out. The template's test support loads it too (ticket 11).
- **"Next steps" run as printed:** commands are printed for the directory `rok` ran in; `rok migrate up` needs no `--dir` after ticket 24.

From the notes on Karibu Pass's tickets 05 and 06 (the rest of those notes are tickets 20, 31, 36, 37 and 38):

- **Constraints map to a field and a message:** `-- rok:constraint <name> <field> "<message>"` anywhere in the schema dir, for a CHECK or a trigger's `RAISE … USING CONSTRAINT`. The engine generates a constraint table, and the templates' error mapping returns a mapped constraint as a 422 with field details. An unmapped one carries PostgreSQL's own message.
- **Actix features mount resources, not scopes:** generated features use `web::resource` only, and `check-arch` warns when a hand-written scope's prefix covers another feature's routes.
- **`rok add feature <name>`:** scaffolds a hand-written feature's files and registers it at the template's Integration points. Each feature contributes one nested `ApiDoc`, so its OpenAPI registration is one line.
- **Shared reads:** `queries/shared/*.sql` generates `db::queries::shared`, read kinds only, callable from any feature.
- **Rust Client bodies:** `Option<Option<T>>` only in `PATCH` bodies; `Default` is derived where every field is optional.
- **A role at a resource:** a template guide recipe for a path-scoped role extractor, built like `AuthUser`.
- **Calls to schema functions are typed:** from the function's `RETURNS`, nullable unless `-- column:` says otherwise.
- **Serving a new table is announced:** `rok gen` prints a loud line for each table it serves for the first time. `[api] include` is an allowlist alternative to `exclude` (not both).
- **Leftover scaffold-once files are removed while unedited:** an edited one is listed with a note.
- **Parameter names:** `BETWEEN` bounds are `<column>_from` and `<column>_to`, and `-- param: $2 from` renames without restating the type.
- **Set-returning functions:** `generate_series` and `unnest` in `FROM` are typed. Batches use `unnest($1::date[])`, documented.
- **Root scripts:** the Monorepo root has a scripts-only `package.json` (`dev:web`, `dev:api`, `test:web`), with no workspaces.

From the notes on Karibu Pass's tickets 07 and 08:

- **A required parameter without restating its type:** `-- param: $2 reviewer!` (ticket 48).
- **Everyday expressions are typed:** comparisons and boolean logic are `bool`, arithmetic takes its operands' type (`numeric` over integers), `CASE` the type its branches share, and `||` is text; each is nullable when an operand is.
- **The repeated role check** is tickets 42 and 44.
- **Permission before validation:** a `check-arch` rule fails a handler whose body extractor comes before its caller extractor, `rok add feature` scaffolds the caller first, and the template docs put role checks in extractors, so a 403 always comes before a 422.
- **rust-analyzer builds apart:** every template ships `.vscode/settings.json` with `"rust-analyzer.cargo.targetDir": true`, so the editor never competes for `cargo test`'s build lock.
- **`UPDATE … FROM` and `DELETE … USING`** are typed like the equivalent join. `-- rok:skip` leaves one query ungenerated on purpose without `--lenient`.
- **Data-modifying CTEs** are typed from their final statement.
- **`-- returns: <RowType>`** reuses an existing row type, failing on any column mismatch.
- **Transaction control in services:** `check-arch` rule 2 allows a `Transaction` and its `begin` (a savepoint), `commit` and `rollback`, and still forbids SQL. The template's `AGENTS.md` documents the `ON CONFLICT … DO NOTHING` + `:execrows` retry, and the templates enable `reqwest`'s `json` feature.
- **A provider that calls back:** the templates gain a `SignedWebhook` extractor (headers and raw body, with a verifier you plug in) as part of the security Capability, and a guide recipe for a provider with real and fake transports chosen by `<NAME>_TRANSPORT`. No vendor is shipped. Tests read queued mail and SMS through the memory transport.

From the notes on Karibu Pass's ticket 09:

- **A query's parameter shape doesn't flip:** `-- rok:params struct|positional` fixes it, and each generated function's doc comment states the default (up to three positional, four or more a `…Params` struct).
- **A contract export that fails to compile says so:** "the App doesn't build after this change: fix the call sites, then run `rok gen` to write the contract and the Clients".
- **Query-parameter enums reach the contract:** generated features register the schemas their query-parameter structs use, and the contract error names the `components(schemas(...))` line a hand-written one needs.
- **Boolean-switched queries:** `$2::bool` with `CASE WHEN $2 THEN … END` in `WHERE` and `ORDER BY` is tested and documented.

From the notes on Karibu Pass's ticket 10 (its member check repeats in tickets 42 and 44; a generated membership extractor waits for a second project to ask):

- **A page on what typed queries support:** one engine docs page with a table of SQL forms, whether each is typed, the override when it isn't, and the test that proves it. Each ticket that adds a form updates it.
- **Results of parameter arithmetic:** `price * $1 AS x` takes the type its parameter took (the numeric column's), nullable when the column is.
- **`cargo run` at the Monorepo root starts the API:** `default-members = ["apps/api"]`, and the root scripts name the package (ticket 50).

From the notes on Karibu Pass's ticket 11 (its query-string enum is ticket 61, now first in the next engine batch):

- **Guards:** two authorization rules copied five times is the second ask, so ticket 44 becomes a `Guard` trait and an `Allowed<G: Guard>` extractor in the templates' security Capability. A project writes each rule once (one `impl Guard`, backed by a shared query), and handlers take `Allowed<Finance>`.
- **A pinned toolchain:** every template ships `rust-toolchain.toml` pinning the stable channel the gates test, so new lints arrive with a rok-templates release, not with a developer's newer toolchain.

From the notes on Karibu Pass's ticket 13 (its nullable aggregates are fixed on main by ticket 31 and reach it with rok-data 0.9.0; its member check is tickets 42 and 44), and from building ticket 61:

- **Clients send `If-Match`:** the contract reader accepts header parameters, and the generated calls take the version, so `rok gen` emits Clients for an App with a versioned table.
- **Descriptions on nullable enum fields:** a property with no description of its own, a `oneOf`/`anyOf` of `null` and one branch, takes that branch's description in every Client.
- **`:page` queries:** one query with its `WHERE` and `ORDER BY` generates a function returning `Page<Row>` (`items`, `total`, `page`, `per_page`): the rows with `LIMIT`/`OFFSET`, and the total from `SELECT count(*)` over the same query.

From the notes on Karibu Pass's ticket 14 (its skipped `INSERT … SELECT` failing loudly is ticket 20, its nullable `count` ticket 31, both on main and in 0.9.0; its member check is ticket 44, unblocked by finishing the gRPC chain: a rok-templates release carrying `grpc`, and `rok init --template grpc` in 0.9.0):

- **`INSERT … SELECT`:** typed like `VALUES`, its column types from the target table, with `ON CONFLICT` and `RETURNING`.
- **Casts keep a typed id, and `EXISTS`:** a cast to a column's own base type keeps its typed id, `EXISTS (…)` is a never-null `bool`, and `-- column:` accepts generated typed ids and enums.
- **Small follow-ups:** check-arch rule 4's message points a pure helper at `shared/`; `rok gen` says the contract export is building the App; the Rust Client's doc for optional sign-in says "when there is one".

From the notes on Karibu Pass's ticket 15 (its untyped `$1` inside a subquery is fixed by ticket 70, and the skip failing the run by ticket 20, both in 0.9.0):

- **`-- rok:down` says where it goes:** the message names the statement and says "immediately after this statement"; the rule stays one block per statement.
- **Guards return a grant:** `Guard::check` returns `Option<Self::Grant>`, and `Allowed<G>` hands the grant to the handler, so one guard can say "member with this role" or "staff" (ticket 44).
- **A transaction helper on a pool:** `rok_data::transaction(&pool, |tx| …)` beside `Db::transaction`, allowed by check-arch rule 2 (ticket 57).

From the notes on Karibu Pass's ticket 16 (its one-statement writes come with tickets 54 and 69 in 0.9.0, then 55 and 49 next; its member check is ticket 44):

- **`jsonb_to_recordset` is typed** from its column definition list (ticket 49).
- **Integer widths in the Clients:** the reader keeps `format`, so `int32` is Rust `i32` (a `smallint` too, since utoipa has no `int16`).
- **Live updates over Topics:** a template Capability. A feature calls `publish(tenant, topic, payload)` over `NOTIFY`, and clients subscribe to one server-sent-events route behind a Guard, so a live board refetches on news instead of polling.

From the notes on Karibu Pass's ticket 17 (its schema-file order is ticket 30 and its duplicated "pending payment" read ticket 42, both in 0.9.0):

- **Set operations:** `UNION`, `UNION ALL`, `INTERSECT` and `EXCEPT` type at the top level and in subqueries, each column from the branches' shared type.
- **A query fills a DTO:** `-- returns:` may name a hand-written `FromRow` type, so the query decodes into it and no row struct is copied field by field (ticket 56).

## Out of Scope

- **Karibu Pass's product work:** phone and SMS sign-in (its ADR-0006), its tickets, removing its sample data, its repo-wide reformat and its own visual-check scripts. A phone sign-in Module may come later.
- **Restyling primitives:** restyling shadcn primitives the designs don't use.
- **Command changes:** moving the single-app templates to `rok.toml` and the front-door commands.
- **Presets:** presets for `--without` lists (ADR 0033 leaves them for later).
- **Web screenshots:** a web screenshot script.

## Further Notes

- **Already fixed:** the `--org` error message the notes ask about already explains the format in current rok ("not a reverse-domain name like `com.example`: two or more dot-separated segments…"); it needs no change.
- **Ordering:** the engine and CLI fixes (tickets 16, 18, 20–26) and rok-ui's (29) don't wait for anything; ticket 01 is done. The rest follows the shipping of the gRPC Template (rok-data 0.8.1, the rok-templates release and `rok init --template grpc`), and goes with the Monorepo `--apps`/`--clients` flags, which are the same kind of init-time choice.
