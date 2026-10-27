//! Aggregates over a relation, loaded beside the rows: how many posts each
//! user has, the sum of their views, whether any exist.

use std::any::TypeId;
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;

use sqlx::postgres::PgRow;
use sqlx::{Decode, Postgres, Row, Type, TypeInfo};

use crate::sql::qualified;
use crate::{Column, Model, Projection, Result, Value};

use super::include::{ErasedState, Identity, IntoInclude, PackedInclude, Relation};
use super::loaded::Loaded;
use super::relations::{distinct_values, key};

/// What an [`Aggregate`] computes per owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Func {
    Count,
    Exists,
    Sum,
    Avg,
    Min,
    Max,
}

impl Func {
    fn name(self) -> &'static str {
        match self {
            Func::Count => "count",
            Func::Exists => "exists",
            Func::Sum => "sum",
            Func::Avg => "avg",
            Func::Min => "min",
            Func::Max => "max",
        }
    }
}

/// One aggregate over relation `R`'s records per owner, read back as `T`:
/// made by [`RelationAggregates`] (`User::POSTS.count()`,
/// `User::POSTS.sum::<i64>(Post::VIEWS)`), loaded by [`Select::with`]
/// beside the rows in one batched `GROUP BY` query, and read with
/// [`Loaded::aggregate`] by the same handle.
///
/// ```ignore
/// let views = User::POSTS.sum::<i64>(Post::VIEWS);
/// let view = User::query().with(User::POSTS.count()).with(views).all(&db).await?;
/// for user in view.rows() {
///     let posts = view.count(User::POSTS, user);
///     let total = view.aggregate(views, user);
/// }
/// ```
///
/// The value type picks the SQL type the aggregate is cast to (`i64` →
/// `INT8`, `f64` → `FLOAT8`, …), so a type that can't hold it fails when
/// the query runs, not when it is read. An owner without records reads as
/// `T::default()`: `0` for a count or a sum, `false` for `exists`, `None`
/// for `avg`, `min` and `max`.
///
/// [`Select::with`]: crate::Select::with
pub struct Aggregate<R, T> {
    relation: R,
    func: Func,
    column: &'static str,
    _value: PhantomData<fn() -> T>,
}

impl<R: Copy, T> Clone for Aggregate<R, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<R: Copy, T> Copy for Aggregate<R, T> {}

impl<R: fmt::Debug, T> fmt::Debug for Aggregate<R, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Aggregate")
            .field("relation", &self.relation)
            .field("func", &self.func.name())
            .field("column", &self.column)
            .finish()
    }
}

/// The aggregate handles every relation constant offers; in the prelude.
pub trait RelationAggregates: Relation {
    /// How many related records each owner has.
    fn count(self) -> Aggregate<Self, i64> {
        Aggregate::new(self, Func::Count, "")
    }

    /// Whether each owner has any related record.
    fn exists(self) -> Aggregate<Self, bool> {
        Aggregate::new(self, Func::Exists, "")
    }

    /// The sum of `column` over each owner's records, `0` without any.
    fn sum<T>(self, column: Column<Self::Target>) -> Aggregate<Self, T> {
        Aggregate::new(self, Func::Sum, column.name())
    }

    /// The average of `column` over each owner's records.
    fn avg<T>(self, column: Column<Self::Target>) -> Aggregate<Self, Option<T>> {
        Aggregate::new(self, Func::Avg, column.name())
    }

    /// The least `column` among each owner's records.
    fn min<T>(self, column: Column<Self::Target>) -> Aggregate<Self, Option<T>> {
        Aggregate::new(self, Func::Min, column.name())
    }

    /// The greatest `column` among each owner's records.
    fn max<T>(self, column: Column<Self::Target>) -> Aggregate<Self, Option<T>> {
        Aggregate::new(self, Func::Max, column.name())
    }
}

impl<R: Relation> RelationAggregates for R {}

/// What a loaded aggregate parks: each owner key's value.
struct State<T>(HashMap<String, T>);

impl<R: Relation, T> Aggregate<R, T> {
    fn new(relation: R, func: Func, column: &'static str) -> Self {
        Self {
            relation,
            func,
            column,
            _value: PhantomData,
        }
    }

    fn identity(&self) -> Identity
    where
        T: 'static,
    {
        (
            TypeId::of::<Aggregate<R, T>>(),
            self.relation.foreign_key_name(),
            self.func.name(),
            self.column,
        )
    }
}

impl<R, T> Aggregate<R, T>
where
    R: Relation,
    T: Type<Postgres> + for<'r> Decode<'r, Postgres> + Default + Send + 'static,
{
    /// The aggregate's SQL: `CAST(<agg> AS <T's type>)`.
    fn projection(&self) -> Projection<R::Target> {
        let target = <R::Target as Model>::TABLE;
        let sql_type = T::type_info();
        let value = match self.func {
            Func::Count => "COUNT(*)".to_owned(),
            Func::Exists => "COUNT(*) > 0".to_owned(),
            Func::Sum => format!("COALESCE(SUM({}), 0)", qualified(target, self.column)),
            Func::Avg => format!("AVG({})", qualified(target, self.column)),
            Func::Min => format!("MIN({})", qualified(target, self.column)),
            Func::Max => format!("MAX({})", qualified(target, self.column)),
        };
        Projection::raw::<Value>(format!("CAST({value} AS {})", sql_type.name()), [])
            .alias("__rok_aggregate")
    }

    fn assemble(rows: Vec<PgRow>) -> Result<State<T>> {
        let mut values = HashMap::new();
        for row in rows {
            let owner = Value::from_row_column(&row, "__rok_key")?;
            let value: T = row.try_get("__rok_aggregate")?;
            if let Some(owner) = key(Some(owner)) {
                values.insert(owner, value);
            }
        }
        Ok(State(values))
    }
}

impl<M, R, T> IntoInclude<M> for Aggregate<R, T>
where
    M: Model,
    R: Relation<Owner = M>,
    T: Type<Postgres> + for<'r> Decode<'r, Postgres> + Default + Send + 'static,
{
    fn into_packed(self) -> PackedInclude<M> {
        let owner_key = self.relation.owner_key();
        let owner = self.relation.owner_of_target();
        let projection = self.projection();
        PackedInclude {
            identity: self.identity(),
            children_sql: Box::new(move |rows: &[M]| {
                self.relation.check()?;
                let ids = distinct_values(rows, owner_key.name());
                let mut select = self
                    .relation
                    .batch_base()
                    .filter(self.relation.batch_filter(ids));
                select.group_by.push(owner.term.clone());
                Ok(select
                    .select((owner.clone().alias("__rok_key"), projection.clone()))
                    .to_sql())
            }),
            assemble: Box::new(|rows| Ok(Box::new(Self::assemble(rows)?) as ErasedState)),
        }
    }
}

impl<M: Model> Loaded<M> {
    /// The value of `aggregate` for `owner`: `None` when the query didn't
    /// ask for it, the default (`0`, `false`, `None`) for an owner without
    /// related records.
    pub fn aggregate<R, T>(&self, aggregate: Aggregate<R, T>, owner: &M) -> Option<T>
    where
        R: Relation<Owner = M>,
        T: Clone + Default + 'static,
    {
        let state = self
            .state(&aggregate.identity())?
            .downcast_ref::<State<T>>()?;
        let owner = key(owner.value_of(aggregate.relation.owner_key().name()));
        Some(
            owner
                .and_then(|owner| state.0.get(&owner).cloned())
                .unwrap_or_default(),
        )
    }

    /// How many records `owner` has through `relation`, when the query
    /// asked for it ([`Select::with_count`](crate::Select::with_count)).
    pub fn count<R: Relation<Owner = M>>(&self, relation: R, owner: &M) -> Option<i64> {
        self.aggregate(relation.count(), owner)
    }

    /// Whether `owner` has any record through `relation`, when the query
    /// asked for it ([`Select::with_exists`](crate::Select::with_exists)).
    pub fn exists<R: Relation<Owner = M>>(&self, relation: R, owner: &M) -> Option<bool> {
        self.aggregate(relation.exists(), owner)
    }
}
