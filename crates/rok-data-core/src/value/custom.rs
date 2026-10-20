//! A value of a type the runtime does not know, kept behind a trait object.

use std::fmt;
use std::sync::Arc;

use sqlx::encode::IsNull;
use sqlx::postgres::{PgArgumentBuffer, PgArguments};
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
