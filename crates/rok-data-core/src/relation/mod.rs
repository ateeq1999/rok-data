//! Relations between models, declared with `#[derive(Model)]` attributes.

use std::fmt;

mod aggregate;
mod belongs_to_many;
mod include;
mod loaded;
mod relations;
mod through;

pub use aggregate::{Aggregate, RelationAggregates};
pub use belongs_to_many::BelongsToMany;
pub use include::{Include, IntoInclude, PackedInclude, Relation};
pub use loaded::{Loaded, Many, One};
pub use relations::{BelongsTo, HasMany, HasOne};
pub use through::{HasManyThrough, HasOneThrough};

macro_rules! relation_boilerplate {
    ($($ty:ident<$a:ident, $b:ident>),*) => {
        $(
            impl<$a, $b> Clone for $ty<$a, $b> {
                fn clone(&self) -> Self {
                    *self
                }
            }
            impl<$a, $b> Copy for $ty<$a, $b> {}
            impl<$a, $b> fmt::Debug for $ty<$a, $b> {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.debug_struct(stringify!($ty))
                        .field("foreign_key", &self.foreign_key)
                        .finish()
                }
            }
        )*
    };
}

relation_boilerplate!(BelongsTo<C, P>, HasMany<P, C>, HasOne<P, C>);

impl<O, T: fmt::Debug> fmt::Debug for One<O, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.map.iter()).finish()
    }
}

impl<O, T: fmt::Debug> fmt::Debug for Many<O, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.map.iter()).finish()
    }
}
