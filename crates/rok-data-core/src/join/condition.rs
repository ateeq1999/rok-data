//! The condition a join is made on.

use std::fmt;
use std::marker::PhantomData;

use crate::expr::Cond;
use crate::relation::{BelongsTo, HasMany, HasOne};
use crate::{Column, Model};

/// The `ON` condition of a join (opaque).
#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct JoinCondition(pub(crate) Cond);

/// Something that can be joined: a relation constant or `ColumnA.on(ColumnB)`.
pub trait IntoJoin {
    /// The model already in the query.
    type From: Model;
    /// The model being joined.
    type To: Model;
    #[doc(hidden)]
    fn condition(&self) -> JoinCondition;
    #[doc(hidden)]
    fn multiplies(&self) -> bool;
}

fn eq_columns(
    left: (&'static str, &'static str),
    right: (&'static str, &'static str),
) -> JoinCondition {
    JoinCondition(Cond::Columns {
        left,
        op: "=",
        right,
    })
}

impl<C: Model, P: Model> IntoJoin for BelongsTo<C, P> {
    type From = C;
    type To = P;
    fn condition(&self) -> JoinCondition {
        eq_columns(
            (P::TABLE, P::PRIMARY_KEY),
            (C::TABLE, self.foreign_key().name()),
        )
    }
    fn multiplies(&self) -> bool {
        false
    }
}

impl<P: Model, C: Model> IntoJoin for HasMany<P, C> {
    type From = P;
    type To = C;
    fn condition(&self) -> JoinCondition {
        eq_columns(
            (C::TABLE, self.foreign_key().name()),
            (P::TABLE, P::PRIMARY_KEY),
        )
    }
    fn multiplies(&self) -> bool {
        true
    }
}

impl<P: Model, C: Model> IntoJoin for HasOne<P, C> {
    type From = P;
    type To = C;
    fn condition(&self) -> JoinCondition {
        eq_columns(
            (C::TABLE, self.foreign_key().name()),
            (P::TABLE, P::PRIMARY_KEY),
        )
    }
    fn multiplies(&self) -> bool {
        false
    }
}

/// An ad-hoc join condition `to = from`, created with [`Column::on`].
pub struct JoinOn<From, To> {
    to: &'static str,
    from: &'static str,
    _models: PhantomData<fn() -> (From, To)>,
}

impl<From: Model, To: Model> IntoJoin for JoinOn<From, To> {
    type From = From;
    type To = To;
    fn condition(&self) -> JoinCondition {
        eq_columns((To::TABLE, self.to), (From::TABLE, self.from))
    }
    /// Unknown cardinality: assume it may multiply rows.
    fn multiplies(&self) -> bool {
        true
    }
}

impl<From, To> fmt::Debug for JoinOn<From, To> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JoinOn")
            .field("to", &self.to)
            .field("from", &self.from)
            .finish()
    }
}

impl<To: Model> Column<To> {
    /// An ad-hoc join condition: join `To` where this column equals `other`
    /// (a column of a model already in the query).
    ///
    /// ```ignore
    /// Post::query().join(Category::ID.on(Post::CATEGORY_ID))
    /// ```
    pub fn on<From: Model>(self, other: Column<From>) -> JoinOn<From, To> {
        JoinOn {
            to: self.name(),
            from: other.name(),
            _models: PhantomData,
        }
    }
}
