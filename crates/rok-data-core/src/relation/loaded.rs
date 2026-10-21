//! The loaded view: the rows and the relations beside them.

use std::any::TypeId;
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;

use crate::Model;

use super::include::{ErasedState, Relation};
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
