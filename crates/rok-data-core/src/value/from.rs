//! The conversions from Rust values into column values.

use super::{CustomType, Value};

impl_array!(String, bool, i16, i32, i64, f32, f64);

#[cfg(feature = "uuid")]
impl_array!(sqlx::types::Uuid);

#[cfg(feature = "chrono")]
impl_array!(
    sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>,
    sqlx::types::chrono::NaiveDate,
);

impl_from! {
    Bool => bool,
    I16 => i16,
    I32 => i32,
    I64 => i64,
    F32 => f32,
    F64 => f64,
    String => String,
    Bytes => Vec<u8>,
}

#[cfg(feature = "uuid")]
impl_from! { Uuid => sqlx::types::Uuid }

#[cfg(feature = "chrono")]
impl_from! {
    DateTime => sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>,
    NaiveDateTime => sqlx::types::chrono::NaiveDateTime,
    NaiveDate => sqlx::types::chrono::NaiveDate,
    NaiveTime => sqlx::types::chrono::NaiveTime,
}

#[cfg(feature = "json")]
impl_from! { Json => serde_json::Value }

#[cfg(feature = "decimal")]
impl_from! { Decimal => sqlx::types::Decimal }

#[cfg(feature = "decimal")]
impl_array!(sqlx::types::Decimal);

impl_from! { Interval => sqlx::postgres::types::PgInterval }

impl_array!(sqlx::postgres::types::PgInterval);

// SQLx's own composite column types without a `Value` variant of their
// own: they go through [`Value::custom`]. Ranges need their element type's
// feature to exist.
impl_value!(sqlx::postgres::types::PgMoney);
impl_value!(
    sqlx::postgres::types::PgRange<i32>,
    sqlx::postgres::types::PgRange<i64>
);
#[cfg(feature = "chrono")]
impl_value!(sqlx::postgres::types::PgTimeTz);
#[cfg(feature = "chrono")]
impl_value!(
    sqlx::postgres::types::PgRange<sqlx::types::chrono::NaiveDate>,
    sqlx::postgres::types::PgRange<sqlx::types::chrono::NaiveDateTime>,
    sqlx::postgres::types::PgRange<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>
);
#[cfg(feature = "decimal")]
impl_value!(sqlx::postgres::types::PgRange<sqlx::types::Decimal>);
#[cfg(feature = "ipnetwork")]
impl_value!(ipnetwork::IpNetwork);
#[cfg(feature = "mac_address")]
impl_value!(mac_address::MacAddress);

// `BYTEA[]`: the element is `Vec<u8>`, which `Value::Bytes` covers, so a
// concrete impl rather than [`CustomType`] wiring (which needs a free
// `From<Option<Vec<u8>>>`).
impl CustomType for Vec<Vec<u8>> {}

impl From<Vec<Vec<u8>>> for Value {
    fn from(v: Vec<Vec<u8>>) -> Self {
        Value::custom(Some(v))
    }
}

impl From<&Vec<Vec<u8>>> for Value {
    fn from(v: &Vec<Vec<u8>>) -> Self {
        Value::custom(Some(v.clone()))
    }
}

#[cfg(feature = "json")]
impl<T: serde::Serialize> From<sqlx::types::Json<T>> for Value {
    fn from(v: sqlx::types::Json<T>) -> Self {
        Value::Json(serde_json::to_value(&v.0).ok())
    }
}

#[cfg(feature = "json")]
impl<T: serde::Serialize> From<&sqlx::types::Json<T>> for Value {
    fn from(v: &sqlx::types::Json<T>) -> Self {
        Value::Json(serde_json::to_value(&v.0).ok())
    }
}

#[cfg(feature = "json")]
impl<T: serde::Serialize> From<&Option<sqlx::types::Json<T>>> for Value {
    fn from(v: &Option<sqlx::types::Json<T>>) -> Self {
        Value::Json(v.as_ref().and_then(|v| serde_json::to_value(&v.0).ok()))
    }
}

#[cfg(feature = "json")]
impl<T: serde::Serialize> From<Option<sqlx::types::Json<T>>> for Value {
    fn from(v: Option<sqlx::types::Json<T>>) -> Self {
        Value::Json(v.and_then(|v| serde_json::to_value(&v.0).ok()))
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::String(Some(v.to_owned()))
    }
}

impl From<Option<&str>> for Value {
    fn from(v: Option<&str>) -> Self {
        Value::String(v.map(str::to_owned))
    }
}

impl From<&[u8]> for Value {
    fn from(v: &[u8]) -> Self {
        Value::Bytes(Some(v.to_vec()))
    }
}

impl From<i8> for Value {
    fn from(v: i8) -> Self {
        Value::I16(Some(v.into()))
    }
}

impl From<u8> for Value {
    fn from(v: u8) -> Self {
        Value::I16(Some(v.into()))
    }
}

impl From<u16> for Value {
    fn from(v: u16) -> Self {
        Value::I32(Some(v.into()))
    }
}

impl From<u32> for Value {
    fn from(v: u32) -> Self {
        Value::I64(Some(v.into()))
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn show<T: std::fmt::Debug>(
            f: &mut std::fmt::Formatter<'_>,
            v: &Option<T>,
        ) -> std::fmt::Result {
            match v {
                Some(v) => write!(f, "{v:?}"),
                None => f.write_str("NULL"),
            }
        }
        match self {
            Value::Bool(v) => show(f, v),
            Value::I16(v) => show(f, v),
            Value::I32(v) => show(f, v),
            Value::I64(v) => show(f, v),
            Value::F32(v) => show(f, v),
            Value::F64(v) => show(f, v),
            Value::String(v) => show(f, v),
            Value::Bytes(v) => show(f, v),
            #[cfg(feature = "uuid")]
            Value::Uuid(v) => show(f, v),
            #[cfg(feature = "chrono")]
            Value::DateTime(v) => show(f, v),
            #[cfg(feature = "chrono")]
            Value::NaiveDateTime(v) => show(f, v),
            #[cfg(feature = "chrono")]
            Value::NaiveDate(v) => show(f, v),
            #[cfg(feature = "chrono")]
            Value::NaiveTime(v) => show(f, v),
            #[cfg(feature = "json")]
            Value::Json(v) => show(f, v),
            #[cfg(feature = "decimal")]
            Value::Decimal(v) => show(f, v),
            Value::Interval(v) => show(f, v),
            Value::Custom(v) => v.0.show(f),
        }
    }
}
