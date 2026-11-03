//! The specialized binaries: `rok-db` (servers, databases, migrations, the
//! live schema) and `rok-gen` (scaffolding and code generation), each the
//! same CLI restricted to its commands (workspace ADR 0032).
//!
//! `rok-data` and rok-v2's `rok` keep every command; a group only narrows
//! what one binary answers and lists.

/// One specialized binary's commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    /// `rok-db`: `server`, `db`, `migrate`, `schema`.
    Db,
    /// `rok-gen`: `init`, `gen`, `generate`, `sqlc`.
    Gen,
}

impl Group {
    /// The commands the group answers, by their names.
    #[must_use]
    pub fn commands(self) -> &'static [&'static str] {
        match self {
            Self::Db => &["server", "db", "migrate", "schema"],
            Self::Gen => &["init", "gen", "generate", "sqlc"],
        }
    }

    /// The group's binary.
    #[must_use]
    pub fn binary(self) -> &'static str {
        match self {
            Self::Db => "rok-db",
            Self::Gen => "rok-gen",
        }
    }

    /// What the group's root help says it is.
    pub(crate) fn about(self) -> &'static str {
        match self {
            Self::Db => {
                "rok-data's database commands: servers, databases, migrations and the live schema"
            }
            Self::Gen => "rok-data's generators: scaffold a project, generate code from SQL",
        }
    }

    /// The other group.
    fn other(self) -> Self {
        match self {
            Self::Db => Self::Gen,
            Self::Gen => Self::Db,
        }
    }

    /// Why `word` (a command's name, aliases resolved) is not this group's,
    /// naming the binary that answers it; `None` when it is this group's or
    /// no command's at all (clap reports those).
    pub(crate) fn refusal(self, program: &str, word: &str) -> Option<String> {
        let other = self.other();
        other.commands().contains(&word).then(|| {
            format!(
                "error: `{program}` has no command `{word}`: it is `{} {word}` (or `rok {word}`)\n\n\
                 Run `{program} --help` for its commands.",
                other.binary()
            )
        })
    }
}
