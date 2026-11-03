# Spec — Editing the schema from the command line

Status: ready-for-agent
Interview outcome in one line: `rok schema column add|remove|change|rename` edit the schema dir's SQL files (never the database) in place, keep every other byte, record renames for the next migration, refuse a removal something still uses, keep the App's API config in step, and take their input as flags or a JSON payload (`-j` inline, a path, or stdin), which every command accepts.

## Problem Statement

A developer changing a table edits a `CREATE TABLE` by hand in the schema dir, then remembers to stage a migration with `rok gen --migration`, and to pass `--rename users.name=full_name` when a column was renamed. Forgetting the flag turns the rename into a drop and an add, which loses the column's data. Removing a column that an index, a foreign key, a query or the API config still names fails later, somewhere else. Nothing in rok makes the edit itself, so scripts and agents can't change a schema without parsing and rewriting SQL themselves.

## Solution

`rok schema column add users.phone text --null` adds the column to the `CREATE TABLE` that defines `users`. It changes only that line, and prints the next command (or, with `--migration`, stages the migration itself).
- **`remove`:** refuses a column that something still uses, naming each use.
- **`rename`:** records itself, so the next migration renames the column instead of dropping it.
- **`change`:** alters only the options it's given.
- **The API config:** `rename` and `remove` keep the App's `rok-data-sqlc.toml` in step.
- **`--dry-run`:** shows the edit as a diff.

Every rok command takes JSON input through `-j`/`--payload` (an inline object, a file path, `@path` or `-` for stdin), so a script can drive any of them with one object. `--json` stays the output switch.

## User Stories

1. As a developer, I want `rok schema column add users.phone text --null` to add the column to the right `CREATE TABLE`, so that I don't hunt through the schema files.
2. As a developer, I want the rest of my schema file left byte for byte as it was, so that my comments and formatting survive and the diff shows one line.
3. As a developer, I want `--after <column>` to place the new line, and a sensible default (before the timestamps), so that the table reads well.
4. As a developer, I want `add` to take `--default`, `--unique`, `--references` (with `--on-delete`) and `--check`, so that one command writes the whole column.
5. As a developer, I want `rok schema column change users.phone --type varchar(20)` to change only what I name, so that I don't restate the column.
6. As a developer, I want `rok schema column rename users.phone=mobile` to make the next migration a rename, so that I never lose a column's data to a forgotten `--rename`.
7. As a developer, I want `rok schema column remove` to refuse a column an index, a foreign key, a `CHECK`, a query or the API config still uses, naming each, so that I find out now, not when something breaks.
8. As a developer, I want `--force` to remove it anyway, so that I decide.
9. As a developer, I want `rename` and `remove` to update the column's entries in `rok-data-sqlc.toml`, so that the config never names a column that isn't there.
10. As a developer, I want the command to print the `rok gen --migration` to run next, with a name suggested from the change, so that I know the next step.
11. As a developer, I want `--migration [NAME]` to stage the migration in the same run, so that one command takes the change to a staged migration.
12. As a developer, I want `--dry-run` to show the edit as a diff and write nothing, so that I can check it first.
13. As a developer in a Monorepo, I want the command to act on the App I'm in, or the one `--app` names, so that I don't pass paths.
14. As a script author, I want every rok command to take its input as one JSON object through `-j`, inline, from a file path or from stdin, so that I don't build long flag lists.
15. As a script author, I want flags given on the command line to override the payload's keys, and an unknown key to fail like an unknown flag, so that a payload is checked as strictly as flags.
16. As a script author, I want `--json` to keep meaning JSON output, so that my existing scripts don't break.

## Implementation Decisions

- **Payload input for every command** (ticket 01):
  - **Forms:** `-j`/`--payload` takes an inline object, a file path, `@path` or `-` (stdin). A value starting with `{` or `[` is JSON; anything else is a path.
  - **Keys:** the object's keys are the command's long flag names (positional arguments by their value names); flags on the command line override them; an unknown key is a usage error naming it.
  - **Output:** `--json` stays the output switch everywhere.
- **The commands:** `rok schema column add|remove|change|rename`, under `rok schema` beside `dump` and `diff`. They say "column", as SQL does; "field" stays an API term.
- **What they edit:** the schema dir's files only, never the database.
  - **Where:** the one `CREATE TABLE` that defines the table; a table defined in no file or in two is an error naming the files.
  - **How:** only the column's line changes, or one line is added before the closing `)` (or where `--after` says, by default before the table's timestamp columns). Every other byte is kept, comments, formatting and line endings included.
- **`add`:**
  - **Type:** the SQL type, positional.
  - **Options:** `--null`/`--not-null`, `--default <SQL>`, `--unique`, `--references <table>[.<column>]`, `--on-delete`, `--check <SQL>` and `--after <column>`.
- **`change`:** takes only the options it changes (`--type`, the same options as `add`); the rest of the column stays as written.
- **`rename`:** edits the file and records the rename as pending in the staging state; the next `rok gen --migration` applies it as `--rename` would, and says so.
- **`remove`:**
  - **Refusal:** it refuses a column an index, a foreign key, a `CHECK`, a query in `queries/` or a per-column list in the API config uses, naming each use; `--force` removes it anyway.
  - **Warning:** the staged `DROP COLUMN` carries the usual data-loss warning.
- **The API config:** `rename` and `remove` update the column's entries in the App's `rok-data-sqlc.toml` (`[api.immutable]`, `[api.hidden]`, `[api.rules]` and the like).
- **After an edit:**
  - **Default:** the command prints the next command, with a migration name suggested from the change (`add_users_phone`).
  - **`--migration [NAME]`:** runs that step in the same run.
  - **`--dry-run`:** prints the edit as a unified diff (schema file and config) and writes nothing; with `--json`, the diff is a field.
- **Finding the project:** starter-feedback ticket 24's rule (the nearest `rok.toml`, the App you're in, or `--app <name>`); a Monorepo with one App that has a schema dir needs no `--app`.

## Testing Decisions

- **Edits:** each command is tested by its output files. The edited schema file must equal the expected one byte for byte (CRLF and comment cases included); a second file in the schema dir is untouched.
- **Rename:** `rename` then `rok gen --migration` stages a `RENAME COLUMN`, not a drop and an add.
- **Remove:** `remove` is refused for each kind of use (index, foreign key, `CHECK`, query, config list), and `--force` goes through.
- **Payload:** tested once for the mechanism (the four forms, override, unknown key) and once per command family that takes it, through the CLI.
- **Dry run:** `--dry-run` writes nothing (file timestamps and contents unchanged).

## Out of Scope

- **Tables:** `rok schema table add|remove|rename` come later in this spec (ticket 06).
- **Indexes and constraints as commands of their own.**
- **The database:** editing it directly; migrations stay the only way a change reaches a database.

## Further Notes

- **Overlap with starter-feedback:** that spec (rok-data#16) has ticket 24's project rule and ticket 21's line-ending normalisation; these commands rely on both.
- **Payload history:** the payload input was agreed earlier as `--payload`/`-j` with `@path` and `-`. Accepting a plain path for `-j` is the one addition here.
