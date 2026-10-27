//! Requesting a relation: the `Relation` trait, `Include`, and the packing of one.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;

use sqlx::postgres::PgRow;

use futures_core::future::BoxFuture;

use crate::exec::Journey;
use crate::sql::Sql;
use crate::{Column, Expr, Model, Order, Projection, Result, Select, Value};

use super::loaded::{Loaded, Many, One};
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

    /// Every record a loaded state holds, for nested includes to load on.
    #[doc(hidden)]
    fn targets(self, state: &Self::Loaded) -> Vec<&Self::Target>;

    /// The targets of the owners whose keys are `ids`: `target_key IN
    /// (ids)` for a direct relation; a through relation reaches them via
    /// its intermediate model.
    #[doc(hidden)]
    fn batch_filter(self, ids: Vec<Value>) -> Expr<Self::Target> {
        self.target_key().is_in(ids)
    }

    /// The batched load's statement: `base` narrowed to the owners' targets,
    /// with whatever [`assemble`](Self::assemble) needs to key each row.
    #[doc(hidden)]
    fn batch_sql(self, base: Select<Self::Target>, ids: Vec<Value>) -> Sql {
        base.filter(self.batch_filter(ids)).to_sql()
    }

    /// The targets of the enclosing query's owner row, for a correlated
    /// `EXISTS`: `target.target_key = owner.owner_key` for a direct
    /// relation.
    #[doc(hidden)]
    fn correlate(self) -> Expr<Self::Target> {
        self.target_key().eq_outer(self.owner_key())
    }

    /// The query a batch over the targets starts from: the target's own
    /// query, or one joined to what keys its rows (a pivot table).
    #[doc(hidden)]
    fn batch_base(self) -> Select<Self::Target> {
        <Self::Target as Model>::query()
    }

    /// A target row's owner key as an expression over the target, to group
    /// aggregates by: the target key for a direct relation.
    #[doc(hidden)]
    fn owner_of_target(self) -> Projection<Self::Target> {
        Projection::from(self.target_key())
    }
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

    fn targets(self, state: &Many<P, C>) -> Vec<&C> {
        state.values().collect()
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

    fn targets(self, state: &One<P, C>) -> Vec<&C> {
        state.values().collect()
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

    fn targets(self, state: &One<C, P>) -> Vec<&P> {
        state.values().collect()
    }
}

/// A relation packaged for [`Select::with`], whose batch load is shaped:
/// filters land in front of the target's implicit scopes, and ordering,
/// limit and offset apply to the whole batch (as [`load from`]). It may
/// carry nested includes, loaded on the batch's records in turn
/// ([`with`](Self::with)).
pub struct Include<R: Relation> {
    relation: R,
    base: Select<R::Target>,
    nested: Vec<PackedInclude<R::Target>>,
    runner: Option<NestedRunner<R>>,
}

impl<R: Relation> Include<R> {
    /// The relation's default batch: the target's fresh query, with its
    /// implicit scopes (tenant, soft delete, default scope) intact.
    pub fn new(relation: R) -> Self {
        Self {
            relation,
            base: <R::Target as Model>::query(),
            nested: Vec::new(),
            runner: None,
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

    /// Loads `include` on this relation's records too, one more batched
    /// query for all of them; read it through
    /// [`Loaded::nested`](crate::Loaded::nested):
    ///
    /// ```ignore
    /// let view = Author::query()
    ///     .with(Author::POSTS.with(Post::COMMENTS))
    ///     .all(&db)
    ///     .await?;
    /// let posts = view.nested(Author::POSTS).unwrap();
    /// for post in view.get(Author::POSTS, &author)? {
    ///     let comments = posts.get(Post::COMMENTS, post)?;
    /// }
    /// ```
    ///
    /// Nested includes nest further (`Post::COMMENTS.with(Comment::AUTHOR)`).
    /// The target rows are cloned into the nested view, hence `Clone`.
    pub fn with<I: IntoInclude<R::Target>>(mut self, include: I) -> Self
    where
        R::Target: Clone,
    {
        self.nested.push(include.into_packed());
        self.runner = Some(run_nested::<R>);
        self
    }

    fn packed<M: Model>(self) -> PackedInclude<M>
    where
        R: Relation<Owner = M>,
    {
        let relation = self.relation;
        let nested = match (self.runner, self.nested) {
            (Some(runner), nested) if !nested.is_empty() => {
                let load: NestedLoad = Box::new(move |journey, state, replica| {
                    runner(relation, journey, state, nested, replica)
                });
                Some(load)
            }
            _ => None,
        };
        PackedInclude {
            nested,
            ..packed(relation, self.base)
        }
    }
}

impl<R: Relation + fmt::Debug> fmt::Debug for Include<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Include")
            .field("relation", &self.relation)
            .field("base", &self.base.to_sql())
            .field("nested", &self.nested.len())
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
    pub(crate) identity: Identity,
    pub(crate) children_sql: ChildrenSql<M>,
    pub(crate) assemble: StateAssembly,
    /// The nested includes' load, run on the assembled state's records.
    pub(crate) nested: Option<NestedLoad>,
}

impl<M: Model> fmt::Debug for PackedInclude<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PackedInclude")
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

/// What a parked state is keyed by: the include's type, its relation's
/// foreign key, and for an aggregate its function and column (empty for a
/// relation; `nested` for a relation's nested view).
pub(crate) type Identity = (TypeId, &'static str, &'static str, &'static str);

/// A relation include's identity.
pub(crate) fn relation_identity<R: Relation>(relation: R) -> Identity {
    (TypeId::of::<R>(), relation.foreign_key_name(), "", "")
}

/// The identity a relation's nested view is parked under.
pub(crate) fn nested_identity((ty, foreign_key, _, _): Identity) -> Identity {
    (ty, foreign_key, "nested", "")
}

pub(crate) type ErasedState = Box<dyn Any + Send>;

/// The SQL for one include's batched children, rendered per owner set.
type ChildrenSql<M> = Box<dyn for<'a> Fn(&'a [M]) -> Result<crate::sql::Sql> + Send>;

/// Rows-to-state assembly, erased so one query can carry several models.
type StateAssembly = Box<dyn Fn(Vec<PgRow>) -> Result<ErasedState> + Send>;

/// The nested includes' load over a relation's assembled state: the
/// target rows as a [`Loaded`] view, its includes loaded, erased.
pub(crate) type NestedLoad = Box<
    dyn for<'j> FnOnce(Journey<'j>, &ErasedState, bool) -> BoxFuture<'j, Result<ErasedState>>
        + Send,
>;

/// [`run_nested`] for one relation type, kept as a plain function pointer
/// so [`Include::with`] (where `Target: Clone` holds) can pick it.
type NestedRunner<R> = for<'j> fn(
    R,
    Journey<'j>,
    &ErasedState,
    Vec<PackedInclude<<R as Relation>::Target>>,
    bool,
) -> BoxFuture<'j, Result<ErasedState>>;

/// The records `state` holds, as a [`Loaded`] view with `nested` loaded.
fn run_nested<'j, R>(
    relation: R,
    journey: Journey<'j>,
    state: &ErasedState,
    nested: Vec<PackedInclude<R::Target>>,
    replica: bool,
) -> BoxFuture<'j, Result<ErasedState>>
where
    R: Relation,
    R::Target: Clone,
{
    let rows: Vec<R::Target> = state
        .downcast_ref::<R::Loaded>()
        .map(|state| relation.targets(state).into_iter().cloned().collect())
        .unwrap_or_default();
    Box::pin(async move {
        let mut view = Loaded::new(rows);
        crate::query::with::load_includes(journey, &mut view, nested, replica).await?;
        Ok(Box::new(view) as ErasedState)
    })
}

fn packed<M: Model, R>(relation: R, base: Select<R::Target>) -> PackedInclude<M>
where
    R: Relation<Owner = M>,
{
    let owner_key = relation.owner_key();
    PackedInclude {
        identity: relation_identity(relation),
        children_sql: Box::new(move |rows: &[M]| {
            relation.check()?;
            let ids = distinct_values(rows, owner_key.name());
            Ok(relation.batch_sql(base.clone(), ids))
        }),
        assemble: Box::new(move |targets| Ok(Box::new(relation.assemble(targets)?) as ErasedState)),
        nested: None,
    }
}
