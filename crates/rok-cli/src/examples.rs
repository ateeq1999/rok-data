//! Worked examples for every command: shown under "Examples" in the
//! command's `--help` (not `-h`), and read by tools that document the CLI
//! (rok-v2's `rok docs`). `tests/examples.rs` parses each one against the
//! grammar, so an example can't drift from what the command accepts.

/// One example: a command line, written as the toolchain's `rok` runs it,
/// and what it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Example {
    /// The command line, starting with `rok`.
    pub command: &'static str,
    /// What the command line does, as a sentence.
    pub what: &'static str,
}

const fn ex(command: &'static str, what: &'static str) -> Example {
    Example { command, what }
}

/// Every command's examples, by its path (`"db create"`, `"migrate up"`).
pub const EXAMPLES: &[(&str, &[Example])] = &[
    (
        "init",
        &[
            ex(
                "rok init shop",
                "Scaffold the actix-web skeleton in ./shop.",
            ),
            ex(
                "rok init shop --api-framework axum",
                "The same skeleton, written for axum.",
            ),
            ex(
                "rok init shop --template monorepo --org com.acme",
                "A monorepo: the axum API with web, mobile and desktop Apps, named under com.acme.",
            ),
            ex(
                "rok init shop --schema schema.sql",
                "Seed the schema directory with an existing file, as schema/0001_init.sql.",
            ),
        ],
    ),
    (
        "gen",
        &[
            ex(
                "rok gen",
                "Generate the data layer and the API features from the schema dir.",
            ),
            ex(
                "rok gen --dry-run",
                "Plan the run and print what it would write; write nothing.",
            ),
            ex(
                "rok gen --migration add_orders",
                "Generate, and stage a migration pair for the schema changes since the last one.",
            ),
            ex(
                "rok gen --rename users=accounts --migration rename_users",
                "Stage a rename instead of a drop and a create.",
            ),
        ],
    ),
    (
        "gen check",
        &[
            ex(
                "rok gen check",
                "Fail (exit 1) when the generated files are out of date; for CI.",
            ),
            ex(
                "rok gen check --json",
                "The same check, as one JSON report.",
            ),
        ],
    ),
    (
        "generate",
        &[
            ex(
                "rok generate --input db --output db-gen",
                "Legacy: generate a model crate in ./db-gen from the .sql files in ./db.",
            ),
            ex(
                "rok generate --migration add_posts",
                "Also write a migration for the schema changes since the last generation.",
            ),
        ],
    ),
    (
        "generate check",
        &[ex(
            "rok generate check",
            "Legacy: fail when the generated crate is out of date with the .sql files.",
        )],
    ),
    (
        "migrate create",
        &[
            ex(
                "rok migrate create create_posts_table",
                "Write the next numbered up/down pair in ./migrations.",
            ),
            ex(
                "rok migrate create add_index --dir db/migrations",
                "The same, in another directory.",
            ),
        ],
    ),
    (
        "migrate status",
        &[
            ex(
                "rok migrate status",
                "List every migration: applied, pending, edited or unknown.",
            ),
            ex(
                "rok migrate status --json",
                "The same, as JSON (with `drift` for edited files).",
            ),
        ],
    ),
    (
        "migrate up",
        &[
            ex(
                "rok migrate up",
                "Apply every pending migration to DATABASE_URL's database.",
            ),
            ex(
                "rok migrate up --database-url postgres://localhost/shop",
                "Apply them to another database.",
            ),
        ],
    ),
    (
        "migrate down",
        &[
            ex("rok migrate down", "Revert the latest migration."),
            ex("rok migrate down --steps 2", "Revert the latest two."),
        ],
    ),
    (
        "db check",
        &[
            ex(
                "rok db check",
                "Connect to DATABASE_URL, ping, and report the server and the pool.",
            ),
            ex("rok db check --json", "The same, as JSON."),
        ],
    ),
    (
        "db inspect",
        &[
            ex("rok db inspect", "List the user tables."),
            ex(
                "rok db inspect --table users",
                "Show one table's columns, indexes and foreign keys.",
            ),
        ],
    ),
    (
        "db create",
        &[
            ex(
                "rok db create shop_dev --server local",
                "Create a database on the registered server `local`.",
            ),
            ex(
                "rok db create shop_test",
                "Create it on DATABASE_URL's server.",
            ),
        ],
    ),
    (
        "db drop",
        &[
            ex(
                "rok db drop shop_test --server local",
                "Drop a database, after typing its name to confirm.",
            ),
            ex(
                "rok db drop shop_test --server local --yes",
                "The same, without the prompt (for scripts).",
            ),
        ],
    ),
    (
        "db query",
        &[
            ex(
                "rok db query 'select count(*) from users'",
                "Run SQL in one transaction; print the rows.",
            ),
            ex(
                "rok db query 'select * from orders limit 5' --server local --json",
                "The same on a registered server, as JSON.",
            ),
        ],
    ),
    (
        "schema dump",
        &[
            ex(
                "rok schema dump",
                "Print every user table as approximate CREATE TABLE DDL.",
            ),
            ex(
                "rok schema dump --database-url postgres://localhost/shop",
                "Dump another database.",
            ),
        ],
    ),
    (
        "schema diff",
        &[
            ex(
                "rok schema diff",
                "Compare the migration journal with the files in ./migrations.",
            ),
            ex(
                "rok schema diff --dir db/migrations",
                "Compare it with another directory.",
            ),
        ],
    ),
    (
        "server add",
        &[
            ex(
                "rok server add local --url postgres://postgres:secret@localhost:5432/postgres",
                "Register a server; the URL's password goes to the OS keychain.",
            ),
            ex(
                "rok server add local --host localhost --user postgres --password-prompt",
                "Register it from flags, typing the password at a prompt.",
            ),
        ],
    ),
    (
        "server update",
        &[
            ex(
                "rok server update local --password-prompt",
                "Store a new password for `local`; its settings stay.",
            ),
            ex(
                "rok server update local --host db.internal --port 6543",
                "Change only the host and the port.",
            ),
        ],
    ),
    (
        "server list",
        &[
            ex(
                "rok server list",
                "List the registered servers and where each password comes from.",
            ),
            ex("rok server list --json", "The same, as JSON."),
        ],
    ),
    (
        "server remove",
        &[
            ex(
                "rok server remove local",
                "Forget `local` and its keychain password, after typing its name.",
            ),
            ex(
                "rok server remove local --yes",
                "The same, without the prompt.",
            ),
        ],
    ),
];

/// The examples of the command at `path` (`"db create"`), if it has any.
#[must_use]
pub fn examples(path: &str) -> Option<&'static [Example]> {
    EXAMPLES
        .iter()
        .find(|(at, _)| *at == path)
        .map(|(_, examples)| *examples)
}

/// `examples` as a help section: each example's sentence, then its line.
pub(crate) fn render(examples: &[Example]) -> String {
    let mut text = String::from("Examples:\n");
    for (i, example) in examples.iter().enumerate() {
        if i > 0 {
            text.push('\n');
        }
        text.push_str(&format!("  # {}\n  {}\n", example.what, example.command));
    }
    text
}
