//! `shared/grpc.rs` under `framework = "grpc"` (engine ADR 0028): the one
//! place an `AppError` becomes a gRPC status, the caller and tenant an
//! interceptor puts in a request's extensions, and the conversions between
//! a message's fields and the values of the DTOs and the rows.
//!
//! The conversions are two traits, `FromWire` and `IntoWire`, so a
//! generated service reads every field with `read` and writes it with
//! `write` whatever its type. The file holds the impls of the types the
//! features' messages carry, and no more: a project without a JSON column
//! needs no `serde_json` for it. The typed ids and the enums get theirs
//! here too, from their inner type and their variants' order (the proto's
//! enum numbers).

mod templates;

use std::path::PathBuf;

use self::templates::{DATE, JSON, MISSING, TIMESTAMP_NAIVE, TIMESTAMP_TZ, TOOLS};
use super::super::feature::{Feature, Key};
use super::shared_dir;
use crate::ApiConfig;
use crate::generator::{HEADER, MAX_WIDTH};
use crate::schema::{BaseType, Column, EnumType, Shape};

/// rustfmt's default `chain_width`.
const CHAIN_WIDTH: usize = 60;

/// Fails on a column of a type no message field holds yet: an interval, a
/// time with a time zone, money, a range, or a type of its own.
pub fn check(features: &[Feature<'_>]) -> Result<(), String> {
    for feature in features {
        for column in columns(feature) {
            let item = item_of(column);
            let supported = match BaseType::of(item) {
                BaseType::Interval | BaseType::TimeTz | BaseType::Money | BaseType::Range => false,
                BaseType::Other => item == "serde_json::Value",
                _ => true,
            };
            if !supported {
                return Err(format!(
                    "`{}.{}` is a `{item}`, which a gRPC message can't carry yet: leave the table out of `api` (`api.exclude`), or hide the column",
                    feature.table.display_name(),
                    column.sql_name
                ));
            }
        }
    }
    Ok(())
}

/// Every column a feature's messages carry: the row's, the inputs' and
/// the key's.
fn columns<'a>(feature: &Feature<'a>) -> Vec<&'a Column> {
    let mut columns = feature.response_columns();
    columns.extend(super::super::dto::create_columns(feature));
    columns.extend(super::super::dto::update_columns(feature));
    match &feature.key {
        Some(Key::Single(column)) => columns.push(column),
        Some(Key::Composite(key)) => columns.extend(key),
        None => {}
    }
    columns.extend(feature.version);
    columns
}

/// The SQLx type of one of `column`'s values: its item, for an array.
fn item_of(column: &Column) -> &str {
    match Shape::of(&column.sqlx_type) {
        Shape::List(item) if column.base_type() == BaseType::Array => item,
        _ => &column.sqlx_type,
    }
}

/// `shared/grpc.rs`, with the impls of the types `features` carry.
pub fn render(features: &[Feature<'_>], api: &ApiConfig, enums: &[EnumType]) -> (PathBuf, String) {
    let mut kinds: Vec<BaseType> = Vec::new();
    let mut typed_ids: Vec<(String, String)> = Vec::new();
    let mut used_enums: Vec<&EnumType> = Vec::new();
    for column in features.iter().flat_map(columns) {
        let item = item_of(column);
        let kind = BaseType::of(item);
        if !kinds.contains(&kind) {
            kinds.push(kind);
        }
        if let Some(id) = &column.typed_id {
            let path = format!("{}::{}::models::{}", api.db_path, id.module, id.name);
            if !typed_ids.iter().any(|(known, _)| *known == path) {
                typed_ids.push((path, item.to_owned()));
            }
        }
        if kind == BaseType::Enum {
            let name = item.rsplit("::").next().unwrap_or(item);
            if let Some(found) = enums.iter().find(|known| known.rust_name == name)
                && !used_enums
                    .iter()
                    .any(|known| known.rust_name == found.rust_name)
            {
                used_enums.push(found);
            }
        }
    }
    let mut output = format!("{HEADER}\n{TOOLS}");
    for (rust, what) in TEXT_TYPES {
        if kinds.contains(&BaseType::of(rust)) {
            output.push_str(&text_impls(rust, what));
        }
    }
    let times = [BaseType::Date, BaseType::Timestamp, BaseType::TimestampTz];
    let timed = times.iter().any(|kind| kinds.contains(kind));
    if timed || kinds.contains(&BaseType::Other) {
        output.push_str(MISSING);
    }
    if timed {
        output.push_str(TIMESTAMP_TZ);
        output.push_str(&required(TIMESTAMP_TZ_TYPE, TIMESTAMP));
        if kinds.contains(&BaseType::Timestamp) {
            output.push_str(TIMESTAMP_NAIVE);
            output.push_str(&required("chrono::NaiveDateTime", TIMESTAMP));
        }
        if kinds.contains(&BaseType::Date) {
            output.push_str(DATE);
            output.push_str(&required("chrono::NaiveDate", TIMESTAMP));
        }
    }
    if kinds.contains(&BaseType::Other) {
        output.push_str(JSON);
        output.push_str(&required("serde_json::Value", "prost_types::Struct"));
    }
    typed_ids.sort();
    for (path, inner) in &typed_ids {
        output.push_str(&typed_id_impls(path, inner));
    }
    used_enums.sort_by(|a, b| a.rust_name.cmp(&b.rust_name));
    for found in used_enums {
        output.push_str(&enum_impls(&api.db_path, found));
    }
    (shared_dir(api).join("grpc.rs"), output)
}

/// The type a message field holding one value of the SQLx type `sqlx` has
/// in Rust, as prost writes it: a message (a time) is always an `Option`.
fn wire_type(sqlx: &str) -> &'static str {
    match BaseType::of(sqlx) {
        BaseType::Integer if sqlx == "i64" => "i64",
        BaseType::Integer => "i32",
        BaseType::Float if sqlx == "f32" => "f32",
        BaseType::Float => "f64",
        BaseType::Bool => "bool",
        BaseType::Bytes => "Vec<u8>",
        BaseType::Date | BaseType::Timestamp | BaseType::TimestampTz => {
            "Option<prost_types::Timestamp>"
        }
        _ => "String",
    }
}

/// `impl {item} for {ty} {`, broken as rustfmt breaks it when too long.
fn head(item: &str, ty: &str) -> String {
    super::super::layout::impl_head(item, ty)
}

/// The impls of a type a message holds as its text.
fn text_impls(rust: &str, what: &str) -> String {
    let map = format!(".map_err(|_| malformed(field, \"{what}\"))");
    // rustfmt breaks a chain longer than its `chain_width`.
    let read = if "wire.parse()".len() + map.len() <= CHAIN_WIDTH {
        format!("        wire.parse(){map}\n")
    } else {
        format!("        wire.parse()\n            {map}\n")
    };
    format!(
        "\n{}    fn from_wire(wire: String, field: &str) -> Result<Self, Status> {{\n{read}    }}\n}}\n\n{}    fn into_wire(self) -> Result<String, Status> {{\n        Ok(self.to_string())\n    }}\n}}\n",
        head("FromWire<String>", rust),
        head("IntoWire<String>", rust),
    )
}

/// The impls reading and writing `rust` in a message field, which is
/// always an `Option` (`wire` is a message): a field left out is
/// `INVALID_ARGUMENT`.
fn required(rust: &str, wire: &str) -> String {
    format!(
        "\n{}    fn from_wire(wire: Option<{wire}>, field: &str) -> Result<Self, Status> {{\n        let wire = wire.ok_or_else(|| missing(field))?;\n        Self::from_wire(wire, field)\n    }}\n}}\n\n{}    fn into_wire(self) -> Result<Option<{wire}>, Status> {{\n        IntoWire::<{wire}>::into_wire(self).map(Some)\n    }}\n}}\n",
        head(&format!("FromWire<Option<{wire}>>"), rust),
        head(&format!("IntoWire<Option<{wire}>>"), rust),
    )
}

/// A typed id's impls: its inner value's, wrapped.
fn typed_id_impls(path: &str, inner: &str) -> String {
    let wire = wire_type(inner);
    // A generic type's path takes a turbofish: `chrono::DateTime::<...>`.
    let inner = inner.replacen('<', "::<", 1);
    let read = format!("        {inner}::from_wire(wire, field).map(Self)\n");
    let read = if read.len() - 1 > MAX_WIDTH {
        format!("        {inner}::from_wire(wire, field)\n            .map(Self)\n")
    } else {
        read
    };
    format!(
        "\n{}    fn from_wire(wire: {wire}, field: &str) -> Result<Self, Status> {{\n{read}    }}\n}}\n\n{}    fn into_wire(self) -> Result<{wire}, Status> {{\n        self.0.into_wire()\n    }}\n}}\n",
        head(&format!("FromWire<{wire}>"), path),
        head(&format!("IntoWire<{wire}>"), path),
    )
}

/// An enum's impls: the proto's numbers, from 1 in the variants' order (0
/// is `<ENUM>_UNSPECIFIED`, no value).
fn enum_impls(db_path: &str, found: &EnumType) -> String {
    let path = format!("{db_path}::types::{}", found.rust_name);
    let mut reads = String::new();
    let mut writes = String::new();
    for (number, variant) in found.variants.iter().enumerate() {
        let number = number + 1;
        reads.push_str(&format!(
            "            {number} => Ok(Self::{}),\n",
            variant.rust_name
        ));
        writes.push_str(&format!(
            "            Self::{} => {number},\n",
            variant.rust_name
        ));
    }
    format!(
        "\n{}    fn from_wire(wire: i32, field: &str) -> Result<Self, Status> {{\n        match wire {{\n{reads}            _ => Err(malformed(field, \"a `{name}`\")),\n        }}\n    }}\n}}\n\n{}    fn into_wire(self) -> Result<i32, Status> {{\n        Ok(match self {{\n{writes}        }})\n    }}\n}}\n",
        head("FromWire<i32>", &path),
        head("IntoWire<i32>", &path),
        name = found.rust_name,
    )
}

/// The types a message holds as their text, and what a field of one holds.
const TEXT_TYPES: [(&str, &str); 5] = [
    ("uuid::Uuid", "a UUID"),
    ("rust_decimal::Decimal", "a decimal number"),
    ("chrono::NaiveTime", "a time of day"),
    ("ipnetwork::IpNetwork", "a network address"),
    ("mac_address::MacAddress", "a MAC address"),
];

const TIMESTAMP: &str = "prost_types::Timestamp";

const TIMESTAMP_TZ_TYPE: &str = "chrono::DateTime<chrono::Utc>";
