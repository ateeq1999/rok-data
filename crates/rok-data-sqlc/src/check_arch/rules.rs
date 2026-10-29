//! The rule texts: what each rule allows, in one place, one rule 1 per
//! framework. `check-arch` checks against these lists, the scaffolded
//! `AGENTS.md` states them (rendered by [`rules_markdown`], with the
//! framework the project is generated for), and
//! `docs/guide/architecture.md` states the default framework's word for
//! word (a test checks it).

use crate::ApiFramework;

/// Rule 1's one framework: the crate its rule polices (`actix_web`, `axum`)
/// and the files allowed to name it. The two backends' emissions mirror
/// each other's file layout, so both allow the same files; only the
/// watched crate — and so the rule's wording — differs.
pub(super) struct Rule1 {
    /// The crate path the generated code imports, as the rule watches it.
    pub(super) watched: &'static str,
    /// The files at the root of `src/` that may use the watched crate.
    pub(super) root: &'static [&'static str],
    /// The feature files that may use the watched crate.
    pub(super) feature: &'static [&'static str],
}

/// The root files rule 1 lets name the watched crate, either backend's:
/// the one shared file table, so the rule's file set is one edit.
const RULE_1_ROOT: &[&str] = &["app.rs", "main.rs", "error.rs"];
/// The feature files rule 1 lets name the watched crate, either backend's.
const RULE_1_FEATURE: &[&str] = &["routes.rs", "handlers.rs"];

/// The wiring's root file, and the directory of its submodules: `src/app/`
/// is the module `app.rs` is, split up, so every rule treats its files as
/// `app.rs`.
pub(super) const WIRING_FILE: &str = "app.rs";
/// See [`WIRING_FILE`].
pub(super) const WIRING_DIR: &str = "app";

/// actix-web's rule 1: the default backend's own rule, watching
/// `actix_web` over the shared file table.
pub(super) const ACTIX_WEB: Rule1 = Rule1 {
    watched: "actix_web",
    root: RULE_1_ROOT,
    feature: RULE_1_FEATURE,
};

/// axum's rule 1: the same laid-out files, watching `axum`. The generated
/// axum code imports crates of `axum` alone (the extractors ride
/// `axum::extract`), so nothing else joins the watch.
pub(super) const AXUM: Rule1 = Rule1 {
    watched: "axum",
    root: RULE_1_ROOT,
    feature: RULE_1_FEATURE,
};

/// The rule 1 of `framework`: what it watches and what may name it.
pub(super) fn rule_1(framework: ApiFramework) -> &'static Rule1 {
    match framework {
        ApiFramework::ActixWeb => &ACTIX_WEB,
        ApiFramework::Axum => &AXUM,
    }
}

/// The files at the root of `src/` that may use `sqlx` (rule 2).
pub(super) const SQLX_ROOT: &[&str] = &["app.rs", "error.rs"];
/// The feature files that may use `sqlx` (rule 2).
pub(super) const SQLX_FEATURE: &[&str] = &["repo.rs", "model.rs"];
/// The feature files that may name the [`SQLX_PORT_TYPES`] (rule 2).
pub(super) const SQLX_PORT_FILES: &[&str] = &["service.rs", "service_api.rs"];
/// The `sqlx` items the [`SQLX_PORT_FILES`] may name (rule 2).
pub(super) const SQLX_PORT_TYPES: &[&str] = &["PgPool", "PgConnection"];

/// `a`, `b` and `c`, each in backticks.
fn listed(items: &[&str]) -> String {
    let quoted: Vec<String> = items.iter().map(|item| format!("`{item}`")).collect();
    match quoted.split_last() {
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
        None => String::new(),
    }
}

/// The infrastructure every rule 1 and 2 list ends with.
const INFRASTRUCTURE: &str = "the features' `generated.rs`, `shared/` and the data layer";

/// The six rules, numbered from 1, as one sentence or paragraph each, for
/// `framework` (rule 1's names). Rule 3 is not checked; the others are.
pub(crate) fn rules(framework: ApiFramework) -> [String; 6] {
    let rule_1 = rule_1(framework);
    let root = |files: &[&str]| {
        files
            .iter()
            .map(|file| match *file {
                WIRING_FILE => format!("`{file}` or `{WIRING_DIR}/`, "),
                _ => format!("`{file}`, "),
            })
            .collect::<String>()
    };
    let ports: Vec<String> = SQLX_PORT_TYPES
        .iter()
        .map(|name| format!("sqlx::{name}"))
        .collect();
    let ports: Vec<&str> = ports.iter().map(String::as_str).collect();
    [
        format!(
            "In a feature, only {} use `{}`. Outside the features, so may \
             {}{INFRASTRUCTURE}.",
            listed(rule_1.feature),
            rule_1.watched,
            root(rule_1.root),
        ),
        format!(
            "In a feature, only {} use `sqlx`, and {} may name {} and nothing else. \
             Outside the features, so may {}{INFRASTRUCTURE}.",
            listed(SQLX_FEATURE),
            listed(SQLX_PORT_FILES),
            listed(&ports),
            root(SQLX_ROOT),
        ),
        "`service.rs` never writes SQL. It calls the data layer's functions and \
         traits from `shared/`."
            .to_owned(),
        "A feature never refers to another feature's modules. It may read any \
         table through the data layer. It writes its own table through \
         `<db_path>::<table>::write`, any table through its own query module, \
         `<db_path>::queries::<feature>`, and another feature's data through \
         that feature's service, behind a port it declares itself, inside the \
         caller's transaction. It never calls another table's `write` or \
         another feature's query module. The data layer is exempt: the schema \
         crosses features."
            .to_owned(),
        "`shared/` never refers to the features.".to_owned(),
        "No `unwrap()`, `expect()`, `panic!`, `todo!`, `println!` or `dbg!`. \
         Every fallible function returns `Result<_, AppError>`."
            .to_owned(),
    ]
}

/// What `check-arch` enforces besides the rules: where the features and the
/// data layer are, the layout, and the test-code exemption.
pub(crate) fn scope() -> String {
    format!(
        "The features are the directories under `api.output` (`src/features/` by \
         default), and the data layer is the module `api.db_path` names \
         (`crate::db`, in `src/db/`, by default). `rok sqlc check-arch` checks \
         rules 1, 2, 4, 5 and 6 and the layout: a feature directory holds only \
         {}; no file or directory is named `utils`, `helpers` or `common`; and \
         `src/` has no top-level directory but the data layer's, `shared/`, the \
         features', `app/` (the wiring, split up) and `bin/`. Test code is exempt from every rule: files under \
         `#![cfg(test)]`, items under `#[cfg(test)]`, `#[test]` and \
         `#[<path>::test]` functions, and `tests/`, which is not read. Each \
         violation prints as `path:line: rule: message`. Rule 3 is yours to keep.",
        listed(super::FEATURE_FILES)
    )
}

/// The rules as a Markdown numbered list wrapped at 76 columns, then
/// [`scope`] as a paragraph: the rules section of the scaffolded `AGENTS.md`,
/// stated with the `AGENTS.md`'s own framework.
pub(crate) fn rules_markdown(framework: ApiFramework) -> String {
    let mut out = String::new();
    for (index, rule) in rules(framework).iter().enumerate() {
        out.push_str(&wrap(&format!("{}. {rule}", index + 1), "   "));
    }
    out.push('\n');
    out.push_str(&wrap(&scope(), ""));
    out.truncate(out.trim_end().len());
    out
}

/// `text` wrapped at 76 columns, continuation lines indented by `indent`,
/// ending with a newline. Never breaks inside backticks.
fn wrap(text: &str, indent: &str) -> String {
    let mut out = String::new();
    let mut line = String::new();
    let mut ticks = 0;
    let mut word = String::new();
    let flush = |line: &mut String, word: &str, out: &mut String| {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > 76 {
            out.push_str(line);
            out.push('\n');
            *line = indent.to_owned();
        } else if !line.is_empty() && !line.ends_with(' ') {
            line.push(' ');
        }
        line.push_str(word);
    };
    for c in text.chars() {
        if c == '`' {
            ticks += 1;
        }
        if c == ' ' && ticks % 2 == 0 {
            flush(&mut line, &word, &mut out);
            word.clear();
        } else {
            word.push(c);
        }
    }
    flush(&mut line, &word, &mut out);
    out.push_str(&line);
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tests read real files, so they share anyhow's `Result`.
    type Result<T> = std::result::Result<T, anyhow::Error>;

    fn normalized(text: &str) -> String {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn rules_name_their_allowed_files() {
        let [one, two, ..] = rules(ApiFramework::ActixWeb);
        assert_eq!(
            one,
            "In a feature, only `routes.rs` and `handlers.rs` use `actix_web`. Outside \
             the features, so may `app.rs` or `app/`, `main.rs`, `error.rs`, the features' \
             `generated.rs`, `shared/` and the data layer."
        );
        assert_eq!(
            two,
            "In a feature, only `repo.rs` and `model.rs` use `sqlx`, and `service.rs` \
             and `service_api.rs` may name `sqlx::PgPool` and `sqlx::PgConnection` and \
             nothing else. Outside the features, so may `app.rs` or `app/`, `error.rs`, the \
             features' `generated.rs`, `shared/` and the data layer."
        );
        // The axum rule allows the same files, and names `axum`: the rule
        // wraps the watched crate's own name everywhere else stays.
        let [one, ..] = rules(ApiFramework::Axum);
        assert_eq!(
            one,
            "In a feature, only `routes.rs` and `handlers.rs` use `axum`. Outside \
             the features, so may `app.rs` or `app/`, `main.rs`, `error.rs`, the features' \
             `generated.rs`, `shared/` and the data layer."
        );
    }

    #[test]
    fn the_markdown_states_every_rule_and_wraps() {
        for framework in [ApiFramework::ActixWeb, ApiFramework::Axum] {
            let markdown = rules_markdown(framework);
            for (index, rule) in rules(framework).iter().enumerate() {
                assert!(
                    normalized(&markdown).contains(&normalized(&format!("{}. {rule}", index + 1))),
                    "rule {}",
                    index + 1
                );
            }
            assert!(normalized(&markdown).contains(&normalized(&scope())));
            for line in markdown.lines() {
                assert!(line.chars().count() <= 76 || !line.contains(' '), "{line}");
            }
        }
    }

    /// The architecture guide states each rule of the default framework, and
    /// what `check-arch` enforces, exactly as `check-arch` does. The axum
    /// wording is the templates' `AGENTS.md`s' own: `init::template`'s test
    /// holds each template to the framework it generates for.
    #[test]
    fn the_architecture_guide_states_the_rules_exactly() -> Result<()> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/docs/guide/architecture.md");
        let guide = normalized(&std::fs::read_to_string(path)?);
        for (index, rule) in rules(ApiFramework::ActixWeb).iter().enumerate() {
            let rule = normalized(&format!("{}. {rule}", index + 1));
            assert!(
                guide.contains(&rule),
                "architecture.md does not say: {rule}"
            );
        }
        let scope = normalized(&scope());
        assert!(
            guide.contains(&scope),
            "architecture.md does not say: {scope}"
        );
        Ok(())
    }
}
