//! The values a column can hold, and how they reach the database.

use std::fmt;
use std::sync::Arc;

use sqlx::encode::IsNull;
use sqlx::postgres::{PgArgumentBuffer, PgArguments};
use sqlx::{Arguments, Encode, Postgres, Type};

use crate::Result;
/// A dynamically typed, bindable SQL value.
///
/// Every variant carries an `Option` so that `NULL`s keep their SQL type,
/// which PostgreSQL requires when binding parameters.
///
/// You rarely build a `Value` yourself: anything implementing `Into<Value>`
/// (integers, floats, strings, `bool`, `Vec<u8>`, `PgInterval`, their `Option`s
/// and, with the matching features, `chrono`, `uuid`, `Decimal` and JSON types) can be passed to
/// the query builder directly.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Value {
    /// `BOOLEAN`
    Bool(Option<bool>),
    /// `SMALLINT` (`INT2`)
    I16(Option<i16>),
    /// `INTEGER` (`INT4`)
    I32(Option<i32>),
    /// `BIGINT` (`INT8`)
    I64(Option<i64>),
    /// `REAL` (`FLOAT4`)
    F32(Option<f32>),
    /// `DOUBLE PRECISION` (`FLOAT8`)
    F64(Option<f64>),
    /// `TEXT` / `VARCHAR`
    String(Option<String>),
    /// `BYTEA`
    Bytes(Option<Vec<u8>>),
    #[cfg(feature = "uuid")]
    /// `UUID`
    Uuid(Option<sqlx::types::Uuid>),
    #[cfg(feature = "chrono")]
    /// `TIMESTAMPTZ`
    DateTime(Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>),
    #[cfg(feature = "chrono")]
    /// `TIMESTAMP`
    NaiveDateTime(Option<sqlx::types::chrono::NaiveDateTime>),
    #[cfg(feature = "chrono")]
    /// `DATE`
    NaiveDate(Option<sqlx::types::chrono::NaiveDate>),
    #[cfg(feature = "chrono")]
    /// `TIME`
    NaiveTime(Option<sqlx::types::chrono::NaiveTime>),
    #[cfg(feature = "json")]
    /// `JSONB` / `JSON`
    Json(Option<serde_json::Value>),
    #[cfg(feature = "decimal")]
    /// `NUMERIC` / `DECIMAL`, as a `rust_decimal` [`Decimal`](sqlx::types::Decimal)
    /// (up to 28 significant digits).
    Decimal(Option<sqlx::types::Decimal>),
    /// `INTERVAL`, as months, days and microseconds.
    Interval(Option<sqlx::postgres::types::PgInterval>),
    /// Any other type sqlx can encode: enums and newtypes from
    /// `#[derive(DbEnum)]` / `#[derive(DbNewtype)]`, or types registered
    /// with [`impl_value!`](crate::impl_value). Build one with [`Value::custom`].
    Custom(CustomValue),
}

impl Value {
    /// Wrap any value sqlx can encode for PostgreSQL. `None` binds a `NULL`
    /// of `T`'s SQL type.
    pub fn custom<T>(value: Option<T>) -> Self
    where
        T: for<'q> Encode<'q, Postgres>
            + Type<Postgres>
            + Clone
            + fmt::Debug
            + Send
            + Sync
            + 'static,
    {
        Value::Custom(CustomValue(Arc::new(value)))
    }

    /// `true` if this value is a (typed) SQL `NULL`.
    pub fn is_null(&self) -> bool {
        match self {
            Value::Bool(v) => v.is_none(),
            Value::I16(v) => v.is_none(),
            Value::I32(v) => v.is_none(),
            Value::I64(v) => v.is_none(),
            Value::F32(v) => v.is_none(),
            Value::F64(v) => v.is_none(),
            Value::String(v) => v.is_none(),
            Value::Bytes(v) => v.is_none(),
            #[cfg(feature = "uuid")]
            Value::Uuid(v) => v.is_none(),
            #[cfg(feature = "chrono")]
            Value::DateTime(v) => v.is_none(),
            #[cfg(feature = "chrono")]
            Value::NaiveDateTime(v) => v.is_none(),
            #[cfg(feature = "chrono")]
            Value::NaiveDate(v) => v.is_none(),
            #[cfg(feature = "chrono")]
            Value::NaiveTime(v) => v.is_none(),
            #[cfg(feature = "json")]
            Value::Json(v) => v.is_none(),
            #[cfg(feature = "decimal")]
            Value::Decimal(v) => v.is_none(),
            Value::Interval(v) => v.is_none(),
            Value::Custom(v) => v.0.is_null(),
        }
    }

    /// Decode column `name` of `row` by its PostgreSQL type (used for keyset
    /// cursors over aliased sort keys).
    pub(crate) fn from_row_column(row: &sqlx::postgres::PgRow, name: &str) -> Result<Value> {
        use sqlx::{Row, TypeInfo, ValueRef};
        let (type_name, is_null) = {
            let raw = row.try_get_raw(name)?;
            (raw.type_info().name().to_owned(), raw.is_null())
        };
        if is_null {
            return Err(crate::Error::InvalidQuery(format!(
                "keyset sort key `{name}` is NULL; keyset pagination needs NOT NULL sort keys"
            )));
        }
        Ok(match type_name.as_str() {
            "BOOL" => Value::from(row.try_get::<bool, _>(name)?),
            "INT2" => Value::from(row.try_get::<i16, _>(name)?),
            "INT4" => Value::from(row.try_get::<i32, _>(name)?),
            "INT8" => Value::from(row.try_get::<i64, _>(name)?),
            "FLOAT4" => Value::from(row.try_get::<f32, _>(name)?),
            "FLOAT8" => Value::from(row.try_get::<f64, _>(name)?),
            "TEXT" | "VARCHAR" | "BPCHAR" | "NAME" | "CITEXT" => {
                Value::from(row.try_get::<String, _>(name)?)
            }
            "BYTEA" => Value::from(row.try_get::<Vec<u8>, _>(name)?),
            #[cfg(feature = "uuid")]
            "UUID" => Value::from(row.try_get::<sqlx::types::Uuid, _>(name)?),
            #[cfg(feature = "chrono")]
            "TIMESTAMPTZ" => Value::from(
                row.try_get::<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>, _>(name)?,
            ),
            #[cfg(feature = "chrono")]
            "TIMESTAMP" => Value::from(row.try_get::<sqlx::types::chrono::NaiveDateTime, _>(name)?),
            #[cfg(feature = "chrono")]
            "DATE" => Value::from(row.try_get::<sqlx::types::chrono::NaiveDate, _>(name)?),
            #[cfg(feature = "chrono")]
            "TIME" => Value::from(row.try_get::<sqlx::types::chrono::NaiveTime, _>(name)?),
            #[cfg(feature = "decimal")]
            "NUMERIC" => Value::from(row.try_get::<sqlx::types::Decimal, _>(name)?),
            "INTERVAL" => Value::from(row.try_get::<sqlx::postgres::types::PgInterval, _>(name)?),
            other => {
                return Err(crate::Error::InvalidQuery(format!(
                    "can't use a `{other}` value as a keyset sort key"
                )));
            }
        })
    }

    /// Encode in PostgreSQL's binary format; `None` for `NULL`.
    pub(crate) fn encode_binary(&self) -> Result<Option<Vec<u8>>> {
        fn enc<T: for<'q> Encode<'q, Postgres>>(
            v: &T,
            buf: &mut PgArgumentBuffer,
        ) -> std::result::Result<IsNull, sqlx::error::BoxDynError> {
            v.encode_by_ref(buf)
        }
        let mut buf = PgArgumentBuffer::default();
        let is_null = match self {
            Value::Bool(v) => enc(v, &mut buf),
            Value::I16(v) => enc(v, &mut buf),
            Value::I32(v) => enc(v, &mut buf),
            Value::I64(v) => enc(v, &mut buf),
            Value::F32(v) => enc(v, &mut buf),
            Value::F64(v) => enc(v, &mut buf),
            Value::String(v) => enc(v, &mut buf),
            Value::Bytes(v) => enc(v, &mut buf),
            #[cfg(feature = "uuid")]
            Value::Uuid(v) => enc(v, &mut buf),
            #[cfg(feature = "chrono")]
            Value::DateTime(v) => enc(v, &mut buf),
            #[cfg(feature = "chrono")]
            Value::NaiveDateTime(v) => enc(v, &mut buf),
            #[cfg(feature = "chrono")]
            Value::NaiveDate(v) => enc(v, &mut buf),
            #[cfg(feature = "chrono")]
            Value::NaiveTime(v) => enc(v, &mut buf),
            #[cfg(feature = "json")]
            Value::Json(v) => enc(v, &mut buf),
            #[cfg(feature = "decimal")]
            Value::Decimal(v) => enc(v, &mut buf),
            Value::Interval(v) => enc(v, &mut buf),
            Value::Custom(v) => v.0.encode(&mut buf),
        }
        .map_err(crate::Error::Encode)?;
        Ok(match is_null {
            IsNull::Yes => None,
            IsNull::No => Some(buf.to_vec()),
        })
    }

    pub(crate) fn bind(self, args: &mut PgArguments) -> Result<()> {
        let res = match self {
            Value::Bool(v) => args.add(v),
            Value::I16(v) => args.add(v),
            Value::I32(v) => args.add(v),
            Value::I64(v) => args.add(v),
            Value::F32(v) => args.add(v),
            Value::F64(v) => args.add(v),
            Value::String(v) => args.add(v),
            Value::Bytes(v) => args.add(v),
            #[cfg(feature = "uuid")]
            Value::Uuid(v) => args.add(v),
            #[cfg(feature = "chrono")]
            Value::DateTime(v) => args.add(v),
            #[cfg(feature = "chrono")]
            Value::NaiveDateTime(v) => args.add(v),
            #[cfg(feature = "chrono")]
            Value::NaiveDate(v) => args.add(v),
            #[cfg(feature = "chrono")]
            Value::NaiveTime(v) => args.add(v),
            #[cfg(feature = "json")]
            Value::Json(v) => args.add(v),
            #[cfg(feature = "decimal")]
            Value::Decimal(v) => args.add(v),
            Value::Interval(v) => args.add(v),
            Value::Custom(v) => v.0.bind(args),
        };
        res.map_err(crate::Error::Encode)
    }
}

/// Make a type usable as a rok-data column value (in models, filters and
/// `set`), given that it already implements sqlx's `Type` and `Encode` for
/// PostgreSQL plus `Clone + Debug`. Implements [`CustomType`] and
/// `From<T>` / `From<&T>` for [`Value`] (`Option<T>` then works too).
///
/// `#[derive(DbEnum)]` and `#[derive(DbNewtype)]` do this for you.
///
/// ```ignore
/// rok_data::impl_value!(my_crate::Money);
/// ```
#[macro_export]
macro_rules! impl_value {
    ($($ty:ty),+ $(,)?) => {$(
        impl $crate::CustomType for $ty {}
        impl ::core::convert::From<$ty> for $crate::Value {
            fn from(v: $ty) -> Self {
                $crate::Value::custom(::core::option::Option::Some(v))
            }
        }
        impl ::core::convert::From<&$ty> for $crate::Value {
            fn from(v: &$ty) -> Self {
                $crate::Value::custom(::core::option::Option::Some(::core::clone::Clone::clone(v)))
            }
        }
    )+};
}

macro_rules! impl_array {
    ($($t:ty),* $(,)?) => {$(
        impl CustomType for Vec<$t> {}
        impl From<Vec<$t>> for Value {
            fn from(v: Vec<$t>) -> Self {
                Value::custom(Some(v))
            }
        }
        impl From<&Vec<$t>> for Value {
            fn from(v: &Vec<$t>) -> Self {
                Value::custom(Some(v.clone()))
            }
        }
        impl From<&[$t]> for Value {
            fn from(v: &[$t]) -> Self {
                Value::custom(Some(v.to_vec()))
            }
        }
    )*};
}

/// Every array whose elements are [`CustomType`] (schema enums, key
/// newtypes, SQLx composites) works as a column value through one blanket.
impl<T> CustomType for Vec<T> where T: CustomType + sqlx::postgres::types::PgHasArrayType {}
impl<T> From<Vec<T>> for Value
where
    T: CustomType + sqlx::postgres::types::PgHasArrayType,
{
    fn from(v: Vec<T>) -> Self {
        Value::custom(Some(v))
    }
}
impl<T> From<&Vec<T>> for Value
where
    T: CustomType + sqlx::postgres::types::PgHasArrayType,
{
    fn from(v: &Vec<T>) -> Self {
        Value::custom(Some(v.clone()))
    }
}

macro_rules! impl_from {
    ($($variant:ident => $ty:ty),* $(,)?) => {$(
        impl From<$ty> for Value {
            fn from(v: $ty) -> Self { Value::$variant(Some(v.into())) }
        }
        impl From<Option<$ty>> for Value {
            fn from(v: Option<$ty>) -> Self { Value::$variant(v.map(Into::into)) }
        }
        impl From<&$ty> for Value {
            fn from(v: &$ty) -> Self { Value::from(v.clone()) }
        }
        impl From<&Option<$ty>> for Value {
            fn from(v: &Option<$ty>) -> Self { Value::from(v.clone()) }
        }
    )*};
}

mod custom;
mod from;

pub use custom::{CustomType, CustomValue};

#[cfg(test)]
mod tests;
