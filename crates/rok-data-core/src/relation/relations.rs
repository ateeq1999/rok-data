//! The relation kinds a model can declare.

use std::collections::HashMap;
use std::marker::PhantomData;

use crate::{Column, Executor, Model, Result, Select, Value};

use super::loaded::{Many, One};

/// Hashable identity of a key value; `None` for SQL `NULL`.
pub(crate) fn key(value: Option<Value>) -> Option<String> {
    value.filter(|v| !v.is_null()).map(|v| v.to_string())
}

/// Relations join on a single primary key column.
pub(crate) fn require_single_key<M: Model>() -> Result<()> {
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
pub(crate) fn distinct_values<M: Model>(records: &[M], column: &str) -> Vec<Value> {
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
    pub(crate) foreign_key: &'static str,
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
    pub(crate) foreign_key: &'static str,
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
    pub(crate) foreign_key: &'static str,
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
