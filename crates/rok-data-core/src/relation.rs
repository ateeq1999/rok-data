//! Relations between models, declared with `#[derive(Model)]` attributes.
//! The parent model must have a single-column primary key.
//!
//! Every relation can be queried lazily for one record, or eager-loaded for
//! many records with a single extra query (avoiding the N+1 problem):
//!
//! ```ignore
//! #[derive(Model)]
//! #[rok(has_many(posts = Post::USER_ID))]
//! struct User { id: i64, name: String }
//!
//! #[derive(Model)]
//! struct Post { id: i64, #[rok(belongs_to = User)] user_id: i64, title: String }
//!
//! // Lazy: one query for one record.
//! let posts = user.posts().order_by(Post::ID).all(&db).await?;
//! let author = post.user().one(&db).await?;
//!
//! // Eager: one query for all records.
//! let users = User::all(&db).await?;
//! let posts = User::POSTS.load(&db, &users).await?;
//! for user in &users {
//!     println!("{} wrote {} posts", user.name, posts.get(user).len());
//! }
//! ```

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;

use sqlx::postgres::PgRow;

use crate::{Column, Executor, Expr, Model, Order, Result, Select, Value};

/// Hashable identity of a key value; `None` for SQL `NULL`.
fn key(value: Option<Value>) -> Option<String> {
    value.filter(|v| !v.is_null()).map(|v| v.to_string())
}

/// Relations join on a single primary key column.
fn require_single_key<M: Model>() -> Result<()> {
    if M::PRIMARY_KEYS.len() == 1 {
        Ok(())
    } else {
        Err(crate::Error::InvalidQuery(format!(
            "relations need a single-column primary key; `{}` has a composite key",
            M::TABLE
        )))
    }
}

/// Distinct, non-null values of `column` across `records`.
fn distinct_values<M: Model>(records: &[M], column: &str) -> Vec<Value> {
    let mut seen = std::collections::HashSet::new();
    records
        .iter()
        .filter_map(|r| r.value_of(column))
        .filter(|v| !v.is_null() && seen.insert(v.to_string()))
        .collect()
}

/// `C` belongs to `P` through the foreign key column `C.fk → P.pk`.
///
/// Generated as an associated constant by `#[rok(belongs_to = P)]` on the
/// foreign key field (`Post::USER` for a field `user_id`).
pub struct BelongsTo<C, P> {
    foreign_key: &'static str,
    _models: PhantomData<fn() -> (C, P)>,
}

impl<C: Model, P: Model> BelongsTo<C, P> {
    /// Declare the relation through `foreign_key`, a column of `C`.
    pub const fn new(foreign_key: Column<C>) -> Self {
        Self {
            foreign_key: foreign_key.name(),
            _models: PhantomData,
        }
    }

    /// The foreign key column on `C`.
    pub const fn foreign_key(&self) -> Column<C> {
        Column::new(self.foreign_key)
    }

    /// Query the parent of `child` (no rows if the foreign key is `NULL`).
    pub fn query(&self, child: &C) -> Select<P> {
        match child.value_of(self.foreign_key).filter(|v| !v.is_null()) {
            Some(fk) => P::filter(P::primary_key_column().eq(fk)),
            None => P::filter(crate::Expr::none()),
        }
    }

    /// Load the parents of every record in `children` with one query.
    pub async fn load<'e, E: Executor<'e>>(
        &self,
        executor: E,
        children: &[C],
    ) -> Result<One<C, P>> {
        require_single_key::<P>()?;
        let ids = distinct_values(children, self.foreign_key);
        let parents = if ids.is_empty() {
            Vec::new()
        } else {
            P::filter(P::primary_key_column().is_in(ids))
                .all(executor)
                .await?
        };
        let map = parents
            .into_iter()
            .filter_map(|p| key(Some(p.primary_key())).map(|k| (k, p)))
            .collect();
        Ok(One::new(map, self.foreign_key))
    }
}

/// `P` has many `C` through the foreign key column `C.fk → P.pk`.
///
/// Generated as an associated constant by
/// `#[rok(has_many(posts = Post::USER_ID))]` on `P` (`User::POSTS`).
pub struct HasMany<P, C> {
    foreign_key: &'static str,
    _models: PhantomData<fn() -> (P, C)>,
}

impl<P: Model, C: Model> HasMany<P, C> {
    /// Declare the relation through `foreign_key`, a column of `C`.
    pub const fn new(foreign_key: Column<C>) -> Self {
        Self {
            foreign_key: foreign_key.name(),
            _models: PhantomData,
        }
    }

    /// The foreign key column on `C`.
    pub const fn foreign_key(&self) -> Column<C> {
        Column::new(self.foreign_key)
    }

    /// Query the children of `parent`.
    pub fn query(&self, parent: &P) -> Select<C> {
        C::filter(self.foreign_key().eq(parent.primary_key()))
    }

    /// Load the children of every record in `parents` with one query.
    pub async fn load<'e, E: Executor<'e>>(
        &self,
        executor: E,
        parents: &[P],
    ) -> Result<Many<P, C>> {
        require_single_key::<P>()?;
        self.load_from(executor, parents, C::query()).await
    }

    /// Like [`load`](Self::load), starting from `base` to filter or order
    /// the children (`Post::order_by(Post::CREATED_AT.desc())`). Limits
    /// apply to the whole result, not per parent.
    pub async fn load_from<'e, E: Executor<'e>>(
        &self,
        executor: E,
        parents: &[P],
        base: Select<C>,
    ) -> Result<Many<P, C>> {
        require_single_key::<P>()?;
        let ids = distinct_values(parents, P::PRIMARY_KEY);
        let children = if ids.is_empty() {
            Vec::new()
        } else {
            base.filter(self.foreign_key().is_in(ids))
                .all(executor)
                .await?
        };
        let mut map: HashMap<String, Vec<C>> = HashMap::new();
        for child in children {
            if let Some(k) = key(child.value_of(self.foreign_key)) {
                map.entry(k).or_default().push(child);
            }
        }
        Ok(Many::new(map, P::PRIMARY_KEY))
    }
}

/// `P` has at most one `C` through the foreign key column `C.fk → P.pk`.
///
/// Generated as an associated constant by
/// `#[rok(has_one(profile = Profile::USER_ID))]` on `P` (`User::PROFILE`).
pub struct HasOne<P, C> {
    foreign_key: &'static str,
    _models: PhantomData<fn() -> (P, C)>,
}

impl<P: Model, C: Model> HasOne<P, C> {
    /// Declare the relation through `foreign_key`, a column of `C`.
    pub const fn new(foreign_key: Column<C>) -> Self {
        Self {
            foreign_key: foreign_key.name(),
            _models: PhantomData,
        }
    }

    /// The foreign key column on `C`.
    pub const fn foreign_key(&self) -> Column<C> {
        Column::new(self.foreign_key)
    }

    /// Query the child of `parent`.
    pub fn query(&self, parent: &P) -> Select<C> {
        C::filter(self.foreign_key().eq(parent.primary_key()))
    }

    /// Load the child of every record in `parents` with one query.
    pub async fn load<'e, E: Executor<'e>>(&self, executor: E, parents: &[P]) -> Result<One<P, C>> {
        require_single_key::<P>()?;
        let ids = distinct_values(parents, P::PRIMARY_KEY);
        let children = if ids.is_empty() {
            Vec::new()
        } else {
            C::filter(self.foreign_key().is_in(ids))
                .all(executor)
                .await?
        };
        let map = children
            .into_iter()
            .filter_map(|c| key(c.value_of(self.foreign_key)).map(|k| (k, c)))
            .collect();
        Ok(One::new(map, P::PRIMARY_KEY))
    }
}

/// Eager-loaded single related records, keyed by owner. Returned by
/// [`BelongsTo::load`] and [`HasOne::load`].
pub struct One<O, T> {
    map: HashMap<String, T>,
    owner_key: &'static str,
    _owner: PhantomData<fn() -> O>,
}

impl<O: Model, T> One<O, T> {
    fn new(map: HashMap<String, T>, owner_key: &'static str) -> Self {
        Self {
            map,
            owner_key,
            _owner: PhantomData,
        }
    }

    /// The record related to `owner`, if any.
    pub fn get(&self, owner: &O) -> Option<&T> {
        key(owner.value_of(self.owner_key)).and_then(|k| self.map.get(&k))
    }

    /// Remove and return the record related to `owner`.
    pub fn take(&mut self, owner: &O) -> Option<T> {
        key(owner.value_of(self.owner_key)).and_then(|k| self.map.remove(&k))
    }

    /// Number of loaded records.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// `true` if nothing was loaded.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Iterate over the loaded records (in no particular order).
    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.map.values()
    }
}

/// Eager-loaded related records grouped by owner. Returned by
/// [`HasMany::load`].
pub struct Many<O, T> {
    map: HashMap<String, Vec<T>>,
    owner_key: &'static str,
    _owner: PhantomData<fn() -> O>,
}

impl<O: Model, T> Many<O, T> {
    fn new(map: HashMap<String, Vec<T>>, owner_key: &'static str) -> Self {
        Self {
            map,
            owner_key,
            _owner: PhantomData,
        }
    }

    /// The records related to `owner` (empty if none).
    pub fn get(&self, owner: &O) -> &[T] {
        key(owner.value_of(self.owner_key))
            .and_then(|k| self.map.get(&k))
            .map_or(&[], Vec::as_slice)
    }

    /// Remove and return the records related to `owner`.
    pub fn take(&mut self, owner: &O) -> Vec<T> {
        key(owner.value_of(self.owner_key))
            .and_then(|k| self.map.remove(&k))
            .unwrap_or_default()
    }

    /// Total number of loaded records.
    pub fn len(&self) -> usize {
        self.map.values().map(Vec::len).sum()
    }

    /// `true` if nothing was loaded.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Iterate over every loaded record (in no particular order).
    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.map.values().flatten()
    }
}

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

type ErasedState = Box<dyn Any + Send>;

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

/// The rows of a `with`-query and the relations loaded beside them. Rows
/// keep their own type; relations are read by the very constants that
/// asked for them, and never panic (an unloaded relation is `None`, not a
/// panic).
pub struct Loaded<M: Model> {
    rows: Vec<M>,
    states: HashMap<(TypeId, &'static str), ErasedState>,
}

impl<M: Model + fmt::Debug> fmt::Debug for Loaded<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Loaded")
            .field("rows", &self.rows.len())
            .field("relations", &self.states.len())
            .finish()
    }
}

impl<M: Model> Loaded<M> {
    /// The fetched rows, before any include parked its state.
    pub(crate) fn new(rows: Vec<M>) -> Self {
        Self {
            rows,
            states: HashMap::new(),
        }
    }

    /// Parks one include's state, keyed by its identity. Repeated
    /// identities overwrite (the relation included twice, last shaping
    /// wins).
    /// Parked states are keyed by the include's identity.
    pub(crate) fn park(&mut self, identity: (TypeId, &'static str), state: ErasedState) {
        self.states.insert(identity, state);
    }

    /// The rows themselves.
    pub fn rows(&self) -> &[M] {
        &self.rows
    }

    /// The rows, dropping the loaded states.
    pub fn into_rows(self) -> Vec<M> {
        self.rows
    }

    /// A loaded relation's state — `None` when `.with()` didn't ask for it
    /// (a caller bug: state exists only for what a query requested).
    pub fn loaded<R>(&self, relation: R) -> Option<&R::Loaded>
    where
        R: Relation<Owner = M> + 'static,
        R::Loaded: 'static,
    {
        self.states
            .get(&(TypeId::of::<R>(), relation.foreign_key_name()))?
            .downcast_ref::<R::Loaded>()
    }

    /// The records related to `owner` through a loaded relation:
    /// `Some` for a loaded include (empty slice for `HasMany`, `None`
    /// inside when an owner has no record on a `One` relation), `None`
    /// when `.with()` didn't ask for the relation.
    pub fn try_get<R>(&self, relation: R, owner: &M) -> Option<R::Read<'_>>
    where
        R: Relation<Owner = M> + 'static,
        R::Loaded: 'static,
    {
        relation.read(self.loaded(relation)?, owner)
    }
}

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
