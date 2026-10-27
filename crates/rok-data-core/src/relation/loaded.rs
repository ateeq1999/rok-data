//! The loaded view: the rows and the relations beside them.

use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;

use crate::exec::Journey;
use crate::{Model, Result};

use super::include::{
    ErasedState, Identity, IntoInclude, Relation, nested_identity, relation_identity,
};
use super::relations::key;

/// The loaded records of a one-relation, keyed by owner: one record per
/// owner. Returned by [`BelongsTo::load`](crate::BelongsTo::load) and
/// [`HasOne::load`](crate::HasOne::load).
pub struct One<O, T> {
    pub(crate) map: HashMap<String, T>,
    owner_key: &'static str,
    _owner: PhantomData<fn() -> O>,
}

impl<O: Model, T> One<O, T> {
    pub(crate) fn new(map: HashMap<String, T>, owner_key: &'static str) -> Self {
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

/// The loaded records of a many-relation, grouped by owner. Returned by
/// [`HasMany::load`](crate::HasMany::load).
pub struct Many<O, T> {
    pub(crate) map: HashMap<String, Vec<T>>,
    owner_key: &'static str,
    _owner: PhantomData<fn() -> O>,
}

impl<O: Model, T> Many<O, T> {
    pub(crate) fn new(map: HashMap<String, Vec<T>>, owner_key: &'static str) -> Self {
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

/// The rows of a `with`-query and the relations loaded beside them. Rows
/// keep their own type; relations are read by the very constants that
/// asked for them, and never panic (an unloaded relation is `None`, not a
/// panic).
pub struct Loaded<M: Model> {
    rows: Vec<M>,
    states: HashMap<Identity, ErasedState>,
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
    pub(crate) fn park(&mut self, identity: Identity, state: ErasedState) {
        self.states.insert(identity, state);
    }

    /// The state parked under `identity`.
    pub(crate) fn state(&self, identity: &Identity) -> Option<&ErasedState> {
        self.states.get(identity)
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
            .get(&relation_identity(relation))?
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

    /// The view of `relation`'s records that its nested includes loaded
    /// on (`.with(Author::POSTS.with(Post::COMMENTS))`): every record the
    /// relation loaded, as rows, with the nested relations beside them.
    /// `None` when the query nested nothing under `relation`.
    pub fn nested<R>(&self, relation: R) -> Option<&Loaded<R::Target>>
    where
        R: Relation<Owner = M> + 'static,
    {
        self.states
            .get(&nested_identity(relation_identity(relation)))?
            .downcast_ref::<Loaded<R::Target>>()
    }

    /// [`try_get`](Self::try_get) that treats an unloaded relation as the
    /// bug it is: [`Error::NotLoaded`](crate::Error::NotLoaded), naming
    /// the relation and the fix, instead of `None`.
    ///
    /// ```ignore
    /// let view = User::query().with(User::POSTS).all(&db).await?;
    /// let posts = view.get(User::POSTS, &user)?;   // &[Post]
    /// let profile = view.get(User::PROFILE, &user); // Err: not loaded
    /// ```
    pub fn get<R>(&self, relation: R, owner: &M) -> Result<R::Read<'_>>
    where
        R: Relation<Owner = M> + 'static,
        R::Loaded: 'static,
    {
        self.try_get(relation, owner)
            .ok_or_else(|| crate::Error::NotLoaded {
                table: M::TABLE,
                foreign_key: relation.foreign_key_name(),
            })
    }

    /// Runs `includes` on this view's rows — one batched query each — and
    /// parks them, replacing an include of the same relation loaded
    /// before (to reload it with other shaping, say):
    ///
    /// ```ignore
    /// view.load(&db, [Include::new(User::POSTS).filter(Post::VIEWS.gte(100))]).await?;
    /// ```
    pub async fn load<'c, S, I, C>(&mut self, session: S, includes: C) -> Result<()>
    where
        S: Into<Journey<'c>>,
        C: IntoIterator<Item = I>,
        I: IntoInclude<M>,
    {
        let includes = includes.into_iter().map(IntoInclude::into_packed).collect();
        crate::query::with::load_includes(session.into(), self, includes, true).await
    }

    /// Runs the includes this view has not loaded yet — one batched query
    /// per still-missing relation. An include already parked (by the
    /// query that fetched the rows, or a previous call) stays as it is;
    /// its shaping is not refetched.
    ///
    /// ```ignore
    /// let mut view = User::filter(User::ID.lt(10))
    ///     .with(User::POSTS)
    ///     .all(&db)
    ///     .await?;
    /// // only the comments run; the posts are already parked
    /// view.load_missing(&db, [Post::COMMENTS]).await?;
    /// ```
    pub async fn load_missing<'c, S, I, C>(&mut self, session: S, includes: C) -> Result<()>
    where
        S: Into<Journey<'c>>,
        C: IntoIterator<Item = I>,
        I: IntoInclude<M>,
    {
        let journey = session.into();
        let pending = includes
            .into_iter()
            .map(IntoInclude::into_packed)
            .filter(|include| !self.states.contains_key(&include.identity))
            .collect();
        crate::query::with::load_includes(journey, self, pending, true).await
    }
}
