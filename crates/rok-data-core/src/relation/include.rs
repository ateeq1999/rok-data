//! Requesting a relation: the `Relation` trait, `Include`, and the packing of one.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;

use sqlx::postgres::PgRow;

use crate::{Column, Expr, Model, Order, Result, Select};

use super::loaded::{Many, One};
use super::relations::{BelongsTo, HasMany, HasOne, distinct_values, key, require_single_key};

/// A relation value — a generated constant of [`HasMany`], [`HasOne`] or
/// [`BelongsTo`] — in the shape [`Select::with`] needs to batch-load it.
///
/// The owner is the model whose rows the query returns; its keys feed the
/// batch. The target is the model on the other end.
pub trait Relation: Copy + Send + Sync + 'static {
    /// The model the relation hangs from.
    type Owner: Model;
    /// The model on the other end.
    type Target: Model;
    /// The loaded state type: [`Many`] for `HasMany`, [`One`] otherwise.
    /// Model values own the state, so it is `Send`.
    type Loaded: Send + 'static;
    /// The related records of one owner as read from a loaded state: a
    /// row slice for `HasMany`, `Option<&Target>` otherwise.
    type Read<'a>;

    /// The foreign-key column that defines the relation: on the child for
    /// `HasMany`/`HasOne` (the target), on the owner for `BelongsTo`. It
    /// also identifies the relation within a query: two relations between
    /// the same models through different foreign keys are two relations.
    fn foreign_key_name(self) -> &'static str;

    /// The column on the owner whose values feed the batch (its primary
    /// key for `HasMany`/`HasOne`, its foreign key for `BelongsTo`).
    fn owner_key(self) -> Column<Self::Owner>;

    /// The column on the target those values are matched against (its
    /// primary key for `BelongsTo`, its foreign key otherwise).
    fn target_key(self) -> Column<Self::Target>;

    /// Rejects composite keys the batch can't key on, before any query runs.
    fn check(self) -> Result<()>;

    /// A loaded state built from the fetched target rows, keyed by the
    /// owners' key-column values. `Many` states group every row; `One`
    /// states keep the last row per key, like their loaders do.
    fn assemble(self, targets: Vec<PgRow>) -> Result<Self::Loaded>;

    /// The records related to `owner` in `state` [`Many::get`] /
    /// [`One::get`]-style: an (empty) slice for `HasMany`, `None` for a
    /// row a `One` state has none of.
    fn read<'a>(self, state: &'a Self::Loaded, owner: &Self::Owner) -> Option<Self::Read<'a>>;
}

impl<P: Model, C: Model> Relation for HasMany<P, C> {
    type Owner = P;
    type Target = C;
    type Loaded = Many<P, C>;
    type Read<'a> = &'a [C];

    fn foreign_key_name(self) -> &'static str {
        self.foreign_key
    }

    fn owner_key(self) -> Column<P> {
        P::primary_key_column()
    }

    fn target_key(self) -> Column<C> {
        self.foreign_key()
    }

    fn check(self) -> Result<()> {
        require_single_key::<P>()
    }

    fn assemble(self, targets: Vec<PgRow>) -> Result<Many<P, C>> {
        let mut map: HashMap<String, Vec<C>> = HashMap::new();
        for row in targets {
            let child = C::from_row(&row).map_err(crate::Error::from)?;
            if let Some(k) = key(child.value_of(self.foreign_key)) {
                map.entry(k).or_default().push(child);
            }
        }
        Ok(Many::new(map, P::PRIMARY_KEY))
    }

    fn read<'a>(self, state: &'a Many<P, C>, owner: &P) -> Option<&'a [C]> {
        Some(state.get(owner))
    }
}

impl<P: Model, C: Model> Relation for HasOne<P, C> {
    type Owner = P;
    type Target = C;
    type Loaded = One<P, C>;
    type Read<'a> = Option<&'a C>;

    fn foreign_key_name(self) -> &'static str {
        self.foreign_key
    }

    fn owner_key(self) -> Column<P> {
        P::primary_key_column()
    }

    fn target_key(self) -> Column<C> {
        self.foreign_key()
    }

    fn check(self) -> Result<()> {
        require_single_key::<P>()
    }

    fn assemble(self, targets: Vec<PgRow>) -> Result<One<P, C>> {
        let mut map: HashMap<String, C> = HashMap::new();
        for row in targets {
            let child = C::from_row(&row).map_err(crate::Error::from)?;
            if let Some(k) = key(child.value_of(self.foreign_key)) {
                map.insert(k, child);
            }
        }
        Ok(One::new(map, P::PRIMARY_KEY))
    }

    fn read<'a>(self, state: &'a One<P, C>, owner: &P) -> Option<Option<&'a C>> {
        Some(state.get(owner))
    }
}

impl<C: Model, P: Model> Relation for BelongsTo<C, P> {
    type Owner = C;
    type Target = P;
    type Loaded = One<C, P>;
    type Read<'a> = Option<&'a P>;

    fn foreign_key_name(self) -> &'static str {
        self.foreign_key
    }

    fn owner_key(self) -> Column<C> {
        self.foreign_key()
    }

    fn target_key(self) -> Column<P> {
        P::primary_key_column()
    }

    fn check(self) -> Result<()> {
        // The parents are keyed by their own primary key.
        require_single_key::<P>()
    }

    fn assemble(self, targets: Vec<PgRow>) -> Result<One<C, P>> {
        let mut map: HashMap<String, P> = HashMap::new();
        for row in targets {
            let parent = P::from_row(&row).map_err(crate::Error::from)?;
            if let Some(k) = key(Some(parent.primary_key())) {
                map.insert(k, parent);
            }
        }
        Ok(One::new(map, self.foreign_key))
    }

    fn read<'a>(self, state: &'a One<C, P>, owner: &C) -> Option<Option<&'a P>> {
        Some(state.get(owner))
    }
}

/// A relation packaged for [`Select::with`], whose batch load is shaped:
/// filters land in front of the target's implicit scopes, and ordering,
/// limit and offset apply to the whole batch (as [`load from`]).
pub struct Include<R: Relation> {
    relation: R,
    base: Select<R::Target>,
}

impl<R: Relation> Include<R> {
    /// The relation's default batch: the target's fresh query, with its
    /// implicit scopes (tenant, soft delete, default scope) intact.
    pub fn new(relation: R) -> Self {
        Self {
            relation,
            base: <R::Target as Model>::query(),
        }
    }

    /// Restricts the records the batch loads.
    pub fn filter(mut self, expr: Expr<R::Target>) -> Self {
        self.base = self.base.filter(expr);
        self
    }

    /// Orders the records the batch loads.
    pub fn order_by(mut self, order: impl Into<Order<R::Target>>) -> Self {
        self.base = self.base.order_by(order);
        self
    }

    /// Caps the records the batch loads. The cap applies to the whole
    /// batch, not per owner — a limit of 10 for three owners is 10
    /// records, however they split (the same semantics `load_from` has).
    pub fn limit(mut self, limit: u64) -> Self {
        self.base = self.base.limit(limit);
        self
    }

    /// Skips that many matched records in the whole batch.
    pub fn offset(mut self, offset: u64) -> Self {
        self.base = self.base.offset(offset);
        self
    }

    fn packed<M: Model>(self) -> PackedInclude<M>
    where
        R: Relation<Owner = M>,
    {
        packed(self.relation, self.base)
    }
}

impl<R: Relation + fmt::Debug> fmt::Debug for Include<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Include")
            .field("relation", &self.relation)
            .field("base", &self.base.to_sql())
            .finish()
    }
}

/// What [`Select::with`] accepts: a bare relation (its default batch), or
/// an [`Include`] with modifiers. Sealed — `relation` values and `Include`
/// values only.
pub trait IntoInclude<Owner: Model> {
    #[doc(hidden)]
    fn into_packed(self) -> PackedInclude<Owner>;
}

impl<Owner: Model, R: Relation<Owner = Owner>> IntoInclude<Owner> for R {
    fn into_packed(self) -> PackedInclude<Owner> {
        Include::new(self).packed()
    }
}

impl<Owner: Model, R: Relation<Owner = Owner>> IntoInclude<Owner> for Include<R> {
    fn into_packed(self) -> PackedInclude<Owner> {
        self.packed()
    }
}

/// A `.with()` call parked on a query: the batched children SQL, rendered
/// when the rows' key values are known, and the fetched-rows-to-state
/// assembly — the relation's genericity erased behind value closures, so
/// one query can carry relations of different child models ([`Loaded`]
/// reads them back typed).
#[doc(hidden)]
pub struct PackedInclude<M: Model> {
    /// Identity (the relation type plus its defining foreign key) for the
    /// state registry. Two inclusions of the same relation overwrite each
    /// other; the last one's shaping wins.
    pub(crate) identity: (TypeId, &'static str),
    pub(crate) children_sql: ChildrenSql<M>,
    pub(crate) assemble: StateAssembly,
}

impl<M: Model> fmt::Debug for PackedInclude<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PackedInclude")
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

pub(crate) type ErasedState = Box<dyn Any + Send>;

/// The SQL for one include's batched children, rendered per owner set.
type ChildrenSql<M> = Box<dyn for<'a> Fn(&'a [M]) -> Result<crate::sql::Sql> + Send>;

/// Rows-to-state assembly, erased so one query can carry several models.
type StateAssembly = Box<dyn Fn(Vec<PgRow>) -> Result<ErasedState> + Send>;

fn packed<M: Model, R>(relation: R, base: Select<R::Target>) -> PackedInclude<M>
where
    R: Relation<Owner = M>,
{
    let owner_key = relation.owner_key();
    let target_key = relation.target_key();
    PackedInclude {
        identity: (TypeId::of::<R>(), relation.foreign_key_name()),
        children_sql: Box::new(move |rows: &[M]| {
            relation.check()?;
            let ids = distinct_values(rows, owner_key.name());
            Ok(base.clone().filter(target_key.is_in(ids)).to_sql())
        }),
        assemble: Box::new(move |targets| Ok(Box::new(relation.assemble(targets)?) as ErasedState)),
    }
}
