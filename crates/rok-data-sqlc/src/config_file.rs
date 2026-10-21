//! `rok-data-sqlc.toml` and the `-j/--json` payload: the two configuration sources
//! the CLI reads besides its flags.
//!
//! Both hold the same options as the command line, in the same shape. Without
//! `-c/--config`, the CLI looks for `rok-data-sqlc.toml` in the current directory
//! and then each parent, like Cargo. Paths in the file resolve relative to the
//! file's directory; paths in a JSON payload resolve relative to the current
//! directory, like flags. The library entry point reads neither (engine ADR 0011).

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;
use serde::de::{self, Deserializer, Visitor};

use crate::report::display_path;
use crate::{Conventions, Derive, Extractor, IdGeneration, RelationshipChange};

mod api_keys;
mod project;

use self::project::RawProject;

pub(crate) use self::api_keys::ApiKeys;

/// The file name looked for during discovery.
pub(crate) const FILE_NAME: &str = "rok-data-sqlc.toml";

/// The canonical project file name (engine ADR 0011), which holds the engine's
/// settings in its `[generate]` section.
pub(crate) const PROJECT_FILE: &str = "rok.toml";

/// The options one configuration source sets, with paths already resolved.
/// `None` (or empty) means the source leaves the option to the sources below
/// it.
#[derive(Debug, Default)]
pub(crate) struct FileConfig {
    pub(crate) schema: Vec<PathBuf>,
    /// The migration history directory the staging writes its pairs into.
    pub(crate) migrations: Option<PathBuf>,
    pub(crate) queries: Vec<PathBuf>,
    pub(crate) output: Option<PathBuf>,
    pub(crate) resource: Option<String>,
    pub(crate) conventions: ConventionKeys,
    /// `None` leaves `derive` to the sources below; a list (even an empty
    /// one) replaces theirs.
    pub(crate) derive: Option<Vec<Derive>>,
    /// Like `derive`: `None` leaves `extractors` to the sources below; a
    /// list (even an empty one) replaces theirs.
    pub(crate) extractors: Option<Vec<Extractor>>,
    pub(crate) relationships: RelationshipKeys,
    /// The `[modules]` renames, by table.
    pub(crate) modules: BTreeMap<String, String>,
    /// The `[api]` keys, with `output` resolved.
    pub(crate) api: ApiKeys,
}

/// The `[relationships]` entries one source sets, by table and then by
/// inferred relationship name.
pub(crate) type RelationshipKeys = BTreeMap<String, BTreeMap<String, RelationshipChange>>;

impl FileConfig {
    /// Layers `self` over `lower`: each option `self` sets wins, the rest come
    /// from `lower`. A list replaces the lower list whole; it never appends.
    pub(crate) fn over(self, lower: FileConfig) -> FileConfig {
        let list = |upper: Vec<PathBuf>, lower: Vec<PathBuf>| {
            if upper.is_empty() { lower } else { upper }
        };
        FileConfig {
            schema: list(self.schema, lower.schema),
            migrations: self.migrations.or(lower.migrations),
            queries: list(self.queries, lower.queries),
            output: self.output.or(lower.output),
            resource: self.resource.or(lower.resource),
            conventions: self.conventions.over(lower.conventions),
            derive: self.derive.or(lower.derive),
            extractors: self.extractors.or(lower.extractors),
            relationships: relationships_over(self.relationships, lower.relationships),
            modules: modules_over(self.modules, lower.modules),
            api: self.api.over(lower.api),
        }
    }

    /// The settings a [`Config`](crate::Config) takes: the file's values,
    /// and the defaults for the rest. The flags override them afterwards.
    pub(crate) fn into_config(self) -> crate::Config {
        let defaults = crate::Config::default();
        crate::Config {
            schema: self.schema,
            migrations: self.migrations.unwrap_or(defaults.migrations),
            queries: self.queries,
            output: self.output.unwrap_or(defaults.output),
            resource: self.resource,
            dry_run: defaults.dry_run,
            check: defaults.check,
            force: defaults.force,
            lenient: defaults.lenient,
            conventions: self.conventions.resolve(),
            derive: self.derive.unwrap_or(defaults.derive),
            extractors: self.extractors.unwrap_or(defaults.extractors),
            relationships: self.relationships,
            modules: self.modules,
            api: self.api.resolve(),
        }
    }
}

/// Layers `upper`'s `[relationships]` entries over `lower`'s, entry by
/// entry: an entry `upper` sets for a relationship replaces `lower`'s.
fn relationships_over(upper: RelationshipKeys, mut lower: RelationshipKeys) -> RelationshipKeys {
    for (table, changes) in upper {
        lower.entry(table).or_default().extend(changes);
    }
    lower
}

/// Layers `upper`'s `[modules]` entries over `lower`'s: an entry `upper`
/// sets for a table replaces `lower`'s.
fn modules_over(
    upper: BTreeMap<String, String>,
    mut lower: BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    lower.extend(upper);
    lower
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    #[serde(default)]
    schema: Option<OneOrMany>,
    /// The migration history directory the staging writes its pairs into.
    #[serde(default)]
    migrations: Option<PathBuf>,
    #[serde(default)]
    queries: Option<OneOrMany>,
    output: Option<PathBuf>,
    resource: Option<String>,
    #[serde(default)]
    conventions: ConventionKeys,
    derive: Option<Vec<DeriveName>>,
    extractors: Option<Vec<ExtractorName>>,
    #[serde(default)]
    relationships: BTreeMap<String, BTreeMap<String, ChangeValue>>,
    #[serde(default)]
    modules: BTreeMap<String, String>,
    #[serde(default)]
    api: ApiKeys,
}

/// A `[relationships]` value: a new name renames the relationship, `false`
/// opts it out.
#[derive(Debug)]
struct ChangeValue(RelationshipChange);

impl<'de> Deserialize<'de> for ChangeValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ChangeVisitor;

        impl Visitor<'_> for ChangeVisitor {
            type Value = ChangeValue;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .write_str("a new relationship name, or `false` to opt the relationship out")
            }

            fn visit_str<E: de::Error>(self, name: &str) -> Result<ChangeValue, E> {
                Ok(ChangeValue(RelationshipChange::Rename(name.to_owned())))
            }

            fn visit_bool<E: de::Error>(self, keep: bool) -> Result<ChangeValue, E> {
                if keep {
                    Err(E::custom(
                        "`true` changes nothing: give a new relationship name, or `false` to opt the relationship out",
                    ))
                } else {
                    Ok(ChangeValue(RelationshipChange::OptOut))
                }
            }
        }

        deserializer.deserialize_any(ChangeVisitor)
    }
}

impl RawFile {
    /// Joins every path onto `base`.
    fn resolve(self, base: &Path) -> FileConfig {
        let resolve = |paths: Option<OneOrMany>| -> Vec<PathBuf> {
            paths
                .map(OneOrMany::into_vec)
                .unwrap_or_default()
                .into_iter()
                .map(|p| base.join(p))
                .collect()
        };
        FileConfig {
            schema: resolve(self.schema),
            migrations: self.migrations.map(|p| base.join(p)),
            queries: resolve(self.queries),
            output: self.output.map(|p| base.join(p)),
            resource: self.resource,
            conventions: self.conventions,
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
            modules: self.modules,
            api: self.api.relative_to(base),
        }
    }
}

/// The `[conventions]` keys one source sets. Keys merge one by one across
/// sources (flags have none); [`ConventionKeys::resolve`] then expands the
/// preset and lets every explicit key override it.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConventionKeys {
    preset: Option<Preset>,
    tenant_column: Option<String>,
    tenant_setting: Option<String>,
    user_setting: Option<String>,
    soft_delete_column: Option<String>,
    managed_columns: Option<Vec<String>>,
    version_column: Option<String>,
    primary_key_ids: Option<IdGenerationName>,
}

/// A `primary_key_ids` value. Deserializing an unknown name fails with the
/// list of known ones; `"none"` turns a lower source's choice off.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum IdGenerationName {
    None,
    UuidV4,
    UuidV7,
    Ulid,
    Cuid2,
}

impl IdGenerationName {
    fn id_generation(self) -> Option<IdGeneration> {
        match self {
            IdGenerationName::None => None,
            IdGenerationName::UuidV4 => Some(IdGeneration::UuidV4),
            IdGenerationName::UuidV7 => Some(IdGeneration::UuidV7),
            IdGenerationName::Ulid => Some(IdGeneration::Ulid),
            IdGenerationName::Cuid2 => Some(IdGeneration::Cuid2),
        }
    }
}

/// A `derive` entry. Deserializing an unknown name fails with the list of
/// known ones.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum DeriveName {
    Serde,
    Validate,
}

impl DeriveName {
    fn derive(self) -> Derive {
        match self {
            DeriveName::Serde => Derive::Serde,
            DeriveName::Validate => Derive::Validate,
        }
    }
}

/// An `extractors` entry. Deserializing an unknown name fails with the list
/// of known ones.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ExtractorName {
    Axum,
    ActixWeb,
}

impl ExtractorName {
    fn extractor(self) -> Extractor {
        match self {
            ExtractorName::Axum => Extractor::Axum,
            ExtractorName::ActixWeb => Extractor::ActixWeb,
        }
    }
}

/// A named bundle of conventions. Deserializing an unknown name fails with
/// the list of known ones.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Preset {
    Rok,
}

impl ConventionKeys {
    /// Layers `self` over `lower`, key by key.
    fn over(self, lower: ConventionKeys) -> ConventionKeys {
        ConventionKeys {
            preset: self.preset.or(lower.preset),
            tenant_column: self.tenant_column.or(lower.tenant_column),
            tenant_setting: self.tenant_setting.or(lower.tenant_setting),
            user_setting: self.user_setting.or(lower.user_setting),
            soft_delete_column: self.soft_delete_column.or(lower.soft_delete_column),
            managed_columns: self.managed_columns.or(lower.managed_columns),
            version_column: self.version_column.or(lower.version_column),
            primary_key_ids: self.primary_key_ids.or(lower.primary_key_ids),
        }
    }

    /// The conventions these keys describe: the preset's values (none
    /// without a preset), each overridden by the explicit key when set. An
    /// empty string turns a preset's single-column convention off.
    pub(crate) fn resolve(self) -> Conventions {
        let mut conventions = match self.preset {
            Some(Preset::Rok) => Conventions::rok(),
            None => Conventions::default(),
        };
        let set = |slot: &mut Option<String>, key: Option<String>| {
            if let Some(value) = key {
                *slot = Some(value).filter(|value| !value.is_empty());
            }
        };
        set(&mut conventions.tenant_column, self.tenant_column);
        set(&mut conventions.tenant_setting, self.tenant_setting);
        set(&mut conventions.user_setting, self.user_setting);
        set(&mut conventions.soft_delete_column, self.soft_delete_column);
        set(&mut conventions.version_column, self.version_column);
        if let Some(managed) = self.managed_columns {
            conventions.managed_columns = managed;
        }
        if let Some(ids) = self.primary_key_ids {
            conventions.primary_key_ids = ids.id_generation();
        }
        conventions
    }
}

/// `schema = "a.sql"` and `schema = ["a.sql", "b.sql"]` are both accepted.
#[derive(Debug, Deserialize)]
#[serde(untagged, expecting = "expected a path or a list of paths")]
enum OneOrMany {
    One(PathBuf),
    Many(Vec<PathBuf>),
}

impl OneOrMany {
    fn into_vec(self) -> Vec<PathBuf> {
        match self {
            Self::One(path) => vec![path],
            Self::Many(paths) => paths,
        }
    }
}

/// The nearest `rok-data-sqlc.toml` in `start` or one of its parents.
pub(crate) fn discover(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .map(|dir| dir.join(FILE_NAME))
        .find(|candidate| candidate.is_file())
}

/// Reads and validates the config file at `path`. Unknown keys are errors
/// naming the key and the file.
pub(crate) fn load(path: &Path) -> Result<FileConfig> {
    let text = read(path)?;
    parse_legacy(&text, path)
}

/// Reads a config file in whichever shape it uses: the legacy flat one
/// (`rok-data-sqlc.toml`), whose keys sit at the top level, or the unified
/// project's (`rok.toml`, engine ADR 0011), whose generation settings sit in its
/// `[generate]` section.
pub(crate) fn load_any(path: &Path) -> Result<FileConfig> {
    let text = read(path)?;
    if is_unified(path, &text) {
        parse_project(&text, path)
    } else {
        parse_legacy(&text, path)
    }
}

/// Reads a config file's text.
fn read(path: &Path) -> Result<String> {
    fs::read_to_string(path)
        .with_context(|| format!("failed to read config file {}", display_path(path)))
}

/// Whether the file uses the unified project shape (engine ADR 0011): the
/// canonical `rok.toml`, or any file with a `[generate]` section.
fn is_unified(path: &Path, text: &str) -> bool {
    if path.file_name().is_some_and(|name| name == PROJECT_FILE) {
        return true;
    }
    toml::from_str::<toml::Table>(text).is_ok_and(|table| table.contains_key("generate"))
}

/// The legacy flat shape: the keys at the top level.
fn parse_legacy(text: &str, path: &Path) -> Result<FileConfig> {
    let raw: RawFile = toml::from_str(text)
        .with_context(|| format!("invalid config file {}", display_path(path)))?;
    Ok(raw.resolve(path.parent().unwrap_or(Path::new(""))))
}

/// The unified project shape: the generation keys in `[generate]`.
fn parse_project(text: &str, path: &Path) -> Result<FileConfig> {
    let raw: RawProject = toml::from_str(text)
        .with_context(|| format!("invalid config file {}", display_path(path)))?;
    Ok(raw.resolve(path.parent().unwrap_or(Path::new(""))))
}

/// Reads and validates a `-j/--json` argument: `-` reads the payload from
/// stdin, `@path` from a file, and anything else is the payload itself.
/// Paths in the payload stay relative to the current directory, wherever the
/// payload came from. Unknown keys and malformed JSON are errors naming the
/// source.
pub(crate) fn load_json(arg: &str) -> Result<FileConfig> {
    let (text, source) = if arg == "-" {
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .context("failed to read the JSON payload from stdin")?;
        (text, "from stdin".to_owned())
    } else if let Some(path) = arg.strip_prefix('@') {
        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read JSON payload file {path}"))?;
        (text, format!("in {path}"))
    } else {
        (arg.to_owned(), "given to --json".to_owned())
    };
    let raw: RawFile =
        serde_json::from_str(&text).with_context(|| format!("invalid JSON payload {source}"))?;
    Ok(raw.resolve(Path::new("")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conventions(toml: &str) -> Conventions {
        let raw: RawFile = toml::from_str(toml).expect("valid config");
        raw.resolve(Path::new("")).conventions.resolve()
    }

    #[test]
    fn rok_preset_with_a_soft_delete_override_uses_the_override() {
        let resolved =
            conventions("[conventions]\npreset = \"rok\"\nsoft_delete_column = \"removed_at\"\n");

        assert_eq!(resolved.soft_delete_column.as_deref(), Some("removed_at"));
        assert_eq!(resolved.tenant_column.as_deref(), Some("organization_id"));
        assert_eq!(
            resolved.tenant_setting.as_deref(),
            Some("app.organization_id")
        );
        assert_eq!(resolved.user_setting.as_deref(), Some("app.user_id"));
        assert_eq!(resolved.managed_columns, ["updated_at", "row_version"]);
        assert_eq!(resolved.version_column.as_deref(), Some("row_version"));
    }

    #[test]
    fn an_empty_name_turns_a_preset_convention_off() {
        let resolved = conventions("[conventions]\npreset = \"rok\"\nversion_column = \"\"\n");

        assert_eq!(resolved.version_column, None);
        assert_eq!(resolved.soft_delete_column.as_deref(), Some("deleted_at"));
    }

    #[test]
    fn primary_key_ids_name_a_strategy_and_unknown_names_fail() {
        let resolved = conventions("[conventions]\nprimary_key_ids = \"uuid_v7\"\n");
        assert_eq!(resolved.primary_key_ids, Some(IdGeneration::UuidV7));
        let off = RawFile::resolve(
            toml::from_str("[conventions]\nprimary_key_ids = \"none\"\n").unwrap(),
            Path::new(""),
        )
        .conventions
        .over(
            RawFile::resolve(
                toml::from_str("[conventions]\nprimary_key_ids = \"ulid\"\n").unwrap(),
                Path::new(""),
            )
            .conventions,
        )
        .resolve();
        assert_eq!(off.primary_key_ids, None, "an upper `none` wins");

        let error = toml::from_str::<RawFile>("[conventions]\nprimary_key_ids = \"snowflake\"\n")
            .unwrap_err()
            .to_string();
        assert!(error.contains("uuid_v7"), "{error}");
    }

    #[test]
    fn no_conventions_section_means_every_convention_is_off() {
        assert_eq!(conventions(""), Conventions::default());
        assert_eq!(conventions("[conventions]\n"), Conventions::default());
        assert_eq!(Conventions::default().managed_columns, Vec::<String>::new());
        assert_eq!(Conventions::default().tenant_column, None);
    }

    /// The legacy file's top level carries the history dir key too.
    #[test]
    fn the_legacy_file_names_the_history_dir() {
        let file = RawFile::resolve(
            toml::from_str("schema = \"schema\"\nmigrations = \"history\"\n").unwrap(),
            Path::new("/project"),
        );
        assert_eq!(file.schema, vec![PathBuf::from("/project/schema")]);
        assert_eq!(file.migrations, Some(PathBuf::from("/project/history")));
    }
}
