//! The unified project config (`rok.toml`, engine ADR 0011): the engine's
//! generation settings in a `[generate]` section, beside the `[api]` and
//! `[conventions]` sections the legacy file holds at the top level.
//!
//! The sections the engine does not own — `[project]`, `[scaffold]` and `[tools]` —
//! are read and ignored: each engine reads only its own sections, and the
//! others belong to the scaffolders (and `[tools]` to rok-tools).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{
    ApiKeys, ChangeValue, ConventionKeys, DeriveName, ExtractorName, FileConfig, OneOrMany,
};

/// The unified project file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawProject {
    /// The project section, owned by the scaffolders.
    #[serde(default)]
    #[allow(dead_code)]
    project: Option<toml::Table>,
    /// The generation settings.
    #[serde(default)]
    generate: Option<GenerationKeys>,
    /// The conventions.
    #[serde(default)]
    conventions: ConventionKeys,
    /// The API settings.
    #[serde(default)]
    api: ApiKeys,
    /// The scaffold settings, owned by the scaffolders.
    #[serde(default)]
    #[allow(dead_code)]
    scaffold: Option<toml::Table>,
    /// The `[tools.<tool>]` settings, owned by rok-tools (workspace ADR 0026).
    #[serde(default)]
    #[allow(dead_code)]
    tools: Option<toml::Table>,
}

/// The `[generate]` section: the engine's generation settings. The keys are
/// the ones the legacy file holds at the top level, under the same names.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GenerationKeys {
    /// The schema inputs.
    schema: Option<OneOrMany>,
    /// The migration history directory the staging writes its pairs into.
    migrations: Option<PathBuf>,
    /// The annotated query files or directories.
    queries: Option<OneOrMany>,
    /// The directory the modules are written to.
    output: Option<PathBuf>,
    /// The table to generate, with the resources already generated.
    resource: Option<String>,
    /// The extra derives for the generated types.
    derive: Option<Vec<DeriveName>>,
    /// The HTTP frameworks to generate extractors for.
    extractors: Option<Vec<ExtractorName>>,
    /// The relationship renames and opt-outs, by table.
    #[serde(default)]
    relationships: BTreeMap<String, BTreeMap<String, ChangeValue>>,
    /// The named scopes of the generated models, by table.
    #[serde(default)]
    scopes: BTreeMap<String, BTreeMap<String, String>>,
    /// The module renames, by table.
    #[serde(default)]
    modules: BTreeMap<String, String>,
}

impl RawProject {
    /// The settings, with the paths resolved against the file's directory.
    pub(super) fn resolve(self, base: &Path) -> FileConfig {
        let mut file = self.generate.unwrap_or_default().resolve(base);
        file.conventions = self.conventions;
        file.api = self.api.relative_to(base);
        file
    }
}

impl GenerationKeys {
    /// The settings, with the paths resolved against the file's directory.
    fn resolve(self, base: &Path) -> FileConfig {
        let paths = |paths: Option<OneOrMany>| -> Vec<PathBuf> {
            paths
                .map(OneOrMany::into_vec)
                .unwrap_or_default()
                .into_iter()
                .map(|path| base.join(path))
                .collect()
        };
        FileConfig {
            schema: paths(self.schema),
            migrations: self.migrations.map(|path| base.join(path)),
            queries: paths(self.queries),
            output: self.output.map(|path| base.join(path)),
            resource: self.resource,
            conventions: ConventionKeys::default(),
            derive: self
                .derive
                .map(|names| names.into_iter().map(DeriveName::derive).collect()),
            extractors: self
                .extractors
                .map(|names| names.into_iter().map(ExtractorName::extractor).collect()),
            relationships: self
                .relationships
                .into_iter()
                .map(|(table, changes)| {
                    let changes = changes
                        .into_iter()
                        .map(|(name, ChangeValue(change))| (name, change))
                        .collect();
                    (table, changes)
                })
                .collect(),
            scopes: self.scopes,
            modules: self.modules,
            api: ApiKeys::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{Conventions, Derive, Extractor, RelationshipChange};

    /// The settings a project file holds, with the paths resolved against
    /// the file's directory.
    fn settings(toml: &str) -> FileConfig {
        let raw: RawProject = toml::from_str(toml).expect("valid project file");
        raw.resolve(Path::new("/project"))
    }

    #[test]
    fn the_generation_keys_live_in_the_generate_section() {
        let file = settings(
            "[generate]\n\
             schema = \"migrations/\"\n\
             output = \"src/db\"\n\
             resource = \"users\"\n\
             derive = [\"serde\"]\n\
             extractors = [\"actix_web\"]\n\
             \n\
             [conventions]\n\
             preset = \"rok\"\n\
             \n\
             [api]\n\
             output = \"src/features\"\n",
        );

        assert_eq!(file.schema, vec![PathBuf::from("/project/migrations")]);
        assert_eq!(file.output, Some(PathBuf::from("/project/src/db")));
        assert_eq!(file.resource.as_deref(), Some("users"));
        assert_eq!(file.derive, Some(vec![Derive::Serde]));
        assert_eq!(file.extractors, Some(vec![Extractor::ActixWeb]));
        assert_eq!(file.conventions.resolve(), Conventions::rok());
        assert_eq!(
            file.api.resolve().map(|api| api.output),
            Some(PathBuf::from("/project/src/features"))
        );
    }

    #[test]
    fn the_sections_the_engine_does_not_own_are_ignored() {
        let file = settings(
            "[project]\nname = \"app\"\n\
             \n\
             [scaffold]\nfeatures = []\n\
             \n\
             [generate]\nschema = \"migrations/\"\n",
        );
        assert_eq!(file.schema, vec![PathBuf::from("/project/migrations")]);
    }

    #[test]
    fn a_scope_names_its_function() {
        let file = settings(
            "[generate]\n\
             [generate.scopes.users]\n\
             famous = \"crate::scopes::users::famous\"\n",
        );
        let users = &file.scopes["users"];
        assert_eq!(users["famous"], "crate::scopes::users::famous".to_owned());
    }

    /// The history dir the staging writes its pairs into: the
    /// `[generate] migrations` key (the truth's `schema` stays what it is).
    #[test]
    fn the_migrations_key_names_the_history_dir() {
        let file = settings("[generate]\nschema = \"schema\"\nmigrations = \"history\"\n");
        assert_eq!(file.schema, vec![PathBuf::from("/project/schema")]);
        assert_eq!(file.migrations, Some(PathBuf::from("/project/history")));
    }

    #[test]
    fn an_unknown_key_in_the_generate_section_is_an_error() {
        let error = toml::from_str::<RawProject>("[generate]\nschem = \"migrations/\"\n")
            .expect_err("the unknown key is refused")
            .to_string();
        assert!(error.contains("schem"), "{error}");
    }

    #[test]
    fn the_tools_section_belongs_to_rok_tools() {
        let raw = toml::from_str::<RawProject>("[tools.loc]\nmax_lines = 600\n");
        assert!(raw.is_ok(), "[tools] is accepted and ignored");
        let error = toml::from_str::<RawProject>("[tool.loc]\nmax_lines = 600\n")
            .expect_err("a misspelled section is still refused")
            .to_string();
        assert!(error.contains("tool"), "{error}");
    }

    #[test]
    fn a_relationship_is_renamed_or_opted_out() {
        let file = settings(
            "[generate]\n\
             [generate.relationships.users]\n\
             created_by_user = \"author\"\n\
             approved_by_user = false\n",
        );
        let users = &file.relationships["users"];
        assert_eq!(
            users["created_by_user"],
            RelationshipChange::Rename("author".into())
        );
        assert_eq!(users["approved_by_user"], RelationshipChange::OptOut);
    }
}
