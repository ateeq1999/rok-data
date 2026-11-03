//! A Resource's `proto/<resource>.proto` under `framework = "grpc"` (engine
//! ADR 0028): the row message, the create and update inputs, the list
//! request and its page, and a service with the unary RPCs the feature
//! serves. Read from the same [`Feature`] model as the HTTP renderers, so
//! the two transports agree on the fields.
//!
//! Every Resource's file shares the project's package, so an SQL enum is
//! nested in the row message that uses it: two Resources with one enum
//! don't define one name twice.

use std::path::{Path, PathBuf};

use heck::{ToShoutySnakeCase, ToUpperCamelCase};

use super::dto::{create_columns, update_columns};
use super::feature::{Endpoint, Feature, Key};
use crate::ApiConfig;
use crate::generator::HEADER;
use crate::schema::{BaseType, Column, EnumType, Shape};

/// The package when the project has no `Cargo.toml` naming its crate.
const FALLBACK_PACKAGE: &str = "api";

/// The well-known types the messages may name, and the file each needs.
const WELL_KNOWN: [(&str, &str); 2] = [
    (
        "google.protobuf.Timestamp",
        "google/protobuf/timestamp.proto",
    ),
    ("google.protobuf.Struct", "google/protobuf/struct.proto"),
];

/// The directory the protos go in: `proto/` at the project's root, the
/// directory holding the `src/` that holds `api.output`; when `api.output`
/// isn't under a `src/`, beside `api.output`.
pub fn proto_dir(api: &ApiConfig) -> PathBuf {
    let parent = api.output.parent().unwrap_or_else(|| Path::new(""));
    let root = if parent.file_name().is_some_and(|name| name == "src") {
        parent.parent().unwrap_or_else(|| Path::new(""))
    } else {
        parent
    };
    root.join("proto")
}

/// One `.proto` per feature, in `proto_dir`, of the package the project's
/// crate names.
pub fn render(
    features: &[Feature<'_>],
    api: &ApiConfig,
    enums: &[EnumType],
) -> Vec<(PathBuf, String)> {
    let dir = proto_dir(api);
    let package = package(&dir);
    features
        .iter()
        .map(|feature| {
            let path = dir.join(format!("{}.proto", feature.module()));
            (path, render_one(feature, &package, enums))
        })
        .collect()
}

/// The package: the crate's name in `Cargo.toml` beside `proto/`, in
/// snake case, versioned like the HTTP prefix (`shop_api.v1`).
fn package(dir: &Path) -> String {
    let manifest = dir
        .parent()
        .map(|root| root.join("Cargo.toml"))
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| text.parse::<toml::Table>().ok());
    let name = manifest
        .as_ref()
        .and_then(|manifest| manifest.get("package")?.get("name")?.as_str())
        .map_or_else(
            || FALLBACK_PACKAGE.to_owned(),
            |name| name.replace('-', "_"),
        );
    format!("{name}.v1")
}

/// A feature's file.
fn render_one(feature: &Feature<'_>, package: &str, enums: &[EnumType]) -> String {
    let row = feature.row();
    let plural = feature.module().to_upper_camel_case();
    let mut body = String::new();
    let mut rpcs = String::new();

    render_row(&mut body, feature, enums);
    let key = key_columns(feature);
    if feature.serves(Endpoint::GetOne) {
        let request = format!("Get{row}Request");
        push_message(
            &mut body,
            &request,
            &fields(&key, row, enums, Presence::Plain),
        );
        push_rpc(&mut rpcs, "Get", &request, row);
    }
    if feature.serves(Endpoint::List) {
        let (request, response) = (
            format!("List{plural}Request"),
            format!("List{plural}Response"),
        );
        push_message(
            &mut body,
            &request,
            &[
                "optional uint32 page".to_owned(),
                "optional uint32 per_page".to_owned(),
            ],
        );
        push_message(
            &mut body,
            &response,
            &[
                format!("repeated {row} items"),
                "int64 total".to_owned(),
                "uint32 page".to_owned(),
                "uint32 per_page".to_owned(),
            ],
        );
        push_rpc(&mut rpcs, "List", &request, &response);
    }
    if feature.serves(Endpoint::Create) {
        let request = format!("Create{row}Request");
        let columns = create_columns(feature);
        let fields: Vec<String> = columns
            .iter()
            .map(|column| {
                let presence = if column.nullable || column.is_optional_in_input() {
                    Presence::Optional
                } else {
                    Presence::Plain
                };
                field(column, row, enums, presence)
            })
            .collect();
        push_message(&mut body, &request, &fields);
        push_rpc(&mut rpcs, "Create", &request, row);
    }
    if feature.serves(Endpoint::Update) {
        let request = format!("Update{row}Request");
        let mut fields = fields(&key, row, enums, Presence::Plain);
        fields.extend(
            update_columns(feature)
                .iter()
                .map(|column| field(column, row, enums, Presence::Optional)),
        );
        push_message(&mut body, &request, &fields);
        push_rpc(&mut rpcs, "Update", &request, row);
    }
    if feature.serves(Endpoint::Remove) {
        let (request, response) = (
            format!("Delete{row}Request"),
            format!("Delete{row}Response"),
        );
        push_message(
            &mut body,
            &request,
            &fields(&key, row, enums, Presence::Plain),
        );
        push_message(&mut body, &response, &[]);
        push_rpc(&mut rpcs, "Delete", &request, &response);
    }

    let mut output = format!(
        "{HEADER}// The `{}` Resource's messages and service (engine ADR 0028).\n\nsyntax = \"proto3\";\n\npackage {package};\n",
        feature.table.display_name()
    );
    let imports: Vec<&str> = WELL_KNOWN
        .iter()
        .filter(|(ty, _)| body.contains(ty))
        .map(|(_, file)| *file)
        .collect();
    if !imports.is_empty() {
        output.push('\n');
        for file in imports {
            output.push_str(&format!("import \"{file}\";\n"));
        }
    }
    output.push_str(&body);
    output.push_str(&format!("\nservice {} {{\n{rpcs}}}\n", feature.service()));
    output
}

/// The row message, with the enums its columns name nested in it.
fn render_row(output: &mut String, feature: &Feature<'_>, enums: &[EnumType]) {
    let row = feature.row();
    let columns = feature.response_columns();
    let mut nested: Vec<&EnumType> = Vec::new();
    let all = columns
        .iter()
        .copied()
        .chain(create_columns(feature))
        .chain(update_columns(feature));
    for column in all {
        if let Some(found) = enum_of(column, enums)
            && !nested
                .iter()
                .any(|known| known.rust_name == found.rust_name)
        {
            nested.push(found);
        }
    }
    let fields: Vec<String> = columns
        .iter()
        .map(|column| field(column, "", enums, Presence::of(column)))
        .collect();
    output.push_str(&format!("\nmessage {row} {{\n"));
    for found in &nested {
        let prefix = found.rust_name.to_shouty_snake_case();
        output.push_str(&format!(
            "  enum {} {{\n    {prefix}_UNSPECIFIED = 0;\n",
            found.rust_name
        ));
        for (number, variant) in found.variants.iter().enumerate() {
            output.push_str(&format!(
                "    {prefix}_{} = {};\n",
                variant.label.to_shouty_snake_case(),
                number + 1
            ));
        }
        output.push_str("  }\n\n");
    }
    push_fields(output, &fields);
    output.push_str("}\n");
}

/// Whether a field is `optional`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Presence {
    Plain,
    Optional,
}

impl Presence {
    /// A row's column: `optional` when nullable.
    fn of(column: &Column) -> Self {
        if column.nullable {
            Presence::Optional
        } else {
            Presence::Plain
        }
    }
}

/// The key's columns: none for a feature without a key.
fn key_columns<'a>(feature: &Feature<'a>) -> Vec<&'a Column> {
    match &feature.key {
        None => Vec::new(),
        Some(Key::Single(column)) => vec![*column],
        Some(Key::Composite(columns)) => columns.clone(),
    }
}

/// `columns` as fields of a message other than the row, every one `presence`.
fn fields(columns: &[&Column], row: &str, enums: &[EnumType], presence: Presence) -> Vec<String> {
    columns
        .iter()
        .map(|column| field(column, row, enums, presence))
        .collect()
}

/// One field, without its number: `optional string title`. An enum is
/// named from outside the row message (`Gadget.Mood`) unless `row` is empty.
fn field(column: &Column, row: &str, enums: &[EnumType], presence: Presence) -> String {
    let name = column.rust_name.trim_start_matches("r#");
    let (item, repeated) = item_of(column);
    let ty = match scalar(item, enums) {
        Scalar::Builtin(ty) => ty.to_owned(),
        Scalar::Enum(found) if row.is_empty() => found.rust_name.clone(),
        Scalar::Enum(found) => format!("{row}.{}", found.rust_name),
    };
    let label = match (repeated, presence) {
        (true, _) => "repeated ",
        (false, Presence::Optional) => "optional ",
        (false, Presence::Plain) => "",
    };
    format!("{label}{ty} {name}")
}

/// The SQLx type of one of `column`'s values, and whether it holds a list
/// of them.
fn item_of(column: &Column) -> (&str, bool) {
    match Shape::of(&column.sqlx_type) {
        Shape::List(item) if column.base_type() == BaseType::Array => (item, true),
        _ => (&column.sqlx_type, false),
    }
}

/// The proto type of one value.
enum Scalar<'e> {
    /// A scalar or well-known type.
    Builtin(&'static str),
    /// An SQL enum, nested in the row message.
    Enum(&'e EnumType),
}

/// The proto type of one value of the SQLx type `sqlx` (engine ADR 0028).
/// The types the ADR doesn't name travel as their text: times of day,
/// intervals, money, ranges and network addresses.
fn scalar<'e>(sqlx: &str, enums: &'e [EnumType]) -> Scalar<'e> {
    Scalar::Builtin(match BaseType::of(sqlx) {
        BaseType::Integer if sqlx == "i64" => "int64",
        BaseType::Integer => "int32",
        BaseType::Float if sqlx == "f32" => "float",
        BaseType::Float => "double",
        BaseType::Bool => "bool",
        BaseType::Bytes => "bytes",
        BaseType::Date | BaseType::Timestamp | BaseType::TimestampTz => "google.protobuf.Timestamp",
        BaseType::Other if sqlx == "serde_json::Value" => "google.protobuf.Struct",
        BaseType::Enum => {
            let name = sqlx.rsplit("::").next().unwrap_or(sqlx);
            match enums.iter().find(|known| known.rust_name == name) {
                Some(found) => return Scalar::Enum(found),
                None => "string",
            }
        }
        _ => "string",
    })
}

/// The enum `column` holds, itself or in a list.
fn enum_of<'e>(column: &Column, enums: &'e [EnumType]) -> Option<&'e EnumType> {
    match scalar(item_of(column).0, enums) {
        Scalar::Enum(found) => Some(found),
        Scalar::Builtin(_) => None,
    }
}

/// A message, its fields numbered from 1.
fn push_message(output: &mut String, name: &str, fields: &[String]) {
    if fields.is_empty() {
        output.push_str(&format!("\nmessage {name} {{}}\n"));
        return;
    }
    output.push_str(&format!("\nmessage {name} {{\n"));
    push_fields(output, fields);
    output.push_str("}\n");
}

fn push_fields(output: &mut String, fields: &[String]) {
    for (number, field) in fields.iter().enumerate() {
        output.push_str(&format!("  {field} = {};\n", number + 1));
    }
}

fn push_rpc(output: &mut String, name: &str, request: &str, response: &str) {
    output.push_str(&format!("  rpc {name}({request}) returns ({response});\n"));
}
