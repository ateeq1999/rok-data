//! A value of a type the runtime does not know, kept behind a trait object.

use std::fmt;
use std::sync::Arc;

use sqlx::encode::IsNull;
use sqlx::postgres::{PgArgumentBuffer, PgArguments, PgTypeInfo};
use sqlx::{Arguments, Encode, Postgres, Type};

use crate::value::Value;

/// A value of a type the runtime does not know, stored through [`Value::custom`].
#[derive(Clone)]
pub struct CustomValue(pub(crate) Arc<dyn DynValue>);

pub(crate) trait DynValue: Send + Sync {
    fn bind(&self, args: &mut PgArguments) -> std::result::Result<(), sqlx::error::BoxDynError>;
    fn encode(
        &self,
        buf: &mut PgArgumentBuffer,
    ) -> std::result::Result<IsNull, sqlx::error::BoxDynError>;
    fn is_null(&self) -> bool;
    fn show(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;
    fn type_info(&self) -> PgTypeInfo;
}

impl<T> DynValue for Option<T>
where
    T: for<'q> Encode<'q, Postgres> + Type<Postgres> + Clone + fmt::Debug + Send + Sync + 'static,
{
    fn bind(&self, args: &mut PgArguments) -> std::result::Result<(), sqlx::error::BoxDynError> {
        args.add(self.clone())
    }

    fn encode(
        &self,
        buf: &mut PgArgumentBuffer,
    ) -> std::result::Result<IsNull, sqlx::error::BoxDynError> {
        Encode::<Postgres>::encode_by_ref(self, buf)
    }

    fn is_null(&self) -> bool {
        self.is_none()
    }

    fn show(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Some(v) => write!(f, "{v:?}"),
            None => f.write_str("NULL"),
        }
    }

    fn type_info(&self) -> PgTypeInfo {
        T::type_info()
    }
}

impl CustomValue {
    /// This value as the built-in [`Value`] its PostgreSQL type is — a key
    /// newtype over `BIGINT` as a [`Value::I64`] — or `None` for a type
    /// with no built-in variant, such as an enum or a composite. A keyset
    /// cursor carries a custom sort key this way.
    pub(crate) fn plain(&self) -> Option<Value> {
        let oid = self.0.type_info().oid()?.0;
        let mut buf = PgArgumentBuffer::default();
        let bytes = match self.0.encode(&mut buf).ok()? {
            IsNull::Yes => None,
            IsNull::No => Some(buf.to_vec()),
        };
        // The binary formats: big-endian integers, a byte per boolean, the
        // UTF-8 text, the raw bytes, a UUID's 16 bytes.
        Some(match oid {
            16 => Value::Bool(fixed::<1>(bytes)?.map(|[b]| b != 0)),
            21 => Value::I16(fixed(bytes)?.map(i16::from_be_bytes)),
            23 => Value::I32(fixed(bytes)?.map(i32::from_be_bytes)),
            20 => Value::I64(fixed(bytes)?.map(i64::from_be_bytes)),
            // `TEXT`, `VARCHAR`, `BPCHAR`
            25 | 1043 | 1042 => Value::String(match bytes {
                Some(bytes) => Some(String::from_utf8(bytes).ok()?),
                None => None,
            }),
            17 => Value::Bytes(bytes),
            #[cfg(feature = "uuid")]
            2950 => Value::Uuid(fixed(bytes)?.map(sqlx::types::Uuid::from_bytes)),
            _ => return None,
        })
    }
}

/// `bytes` as an array of `N`: `Some(None)` for `NULL`, `None` when the
/// length is another.
fn fixed<const N: usize>(bytes: Option<Vec<u8>>) -> Option<Option<[u8; N]>> {
    match bytes {
        Some(bytes) => Some(Some(bytes.try_into().ok()?)),
        None => Some(None),
    }
}

impl fmt::Debug for CustomValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.show(f)
    }
}

impl PartialEq for CustomValue {
    /// Custom values compare by their debug representation.
    fn eq(&self, other: &Self) -> bool {
        format!("{self:?}") == format!("{other:?}")
    }
}

/// Marker for user-defined column types stored through [`Value::custom`].
///
/// Implemented by [`impl_value!`](crate::impl_value), `#[derive(DbEnum)]` and
/// `#[derive(DbNewtype)]`; it unlocks `From<Option<T>>` and
/// `From<&Option<T>>` for [`Value`], which a downstream crate couldn't write
/// itself.
pub trait CustomType:
    for<'q> Encode<'q, Postgres> + Type<Postgres> + Clone + fmt::Debug + Send + Sync + 'static
{
}

impl<T: CustomType> From<Option<T>> for Value {
    fn from(v: Option<T>) -> Self {
        Value::custom(v)
    }
}

impl<T: CustomType> From<&Option<T>> for Value {
    fn from(v: &Option<T>) -> Self {
        Value::custom(v.clone())
    }
}
