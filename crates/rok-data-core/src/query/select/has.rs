//! Rows and their relations on the chain: filtering by them
//! ([`Select::has`], [`Select::where_has`] and their negations, as
//! correlated `EXISTS` subqueries) and counting them beside the rows
//! ([`Select::with_count`], [`Select::with_exists`]).

use crate::query::WithSelect;
use crate::relation::{Relation, RelationAggregates};
use crate::{Expr, Model};

use super::Select;

/// The target's rows related to the enclosing query's row, shaped by
/// `shape`: `target.target_key = owner.owner_key` beside the shaped
/// filters and the target's implicit scopes.
fn related<R: Relation>(
    relation: R,
    shape: impl FnOnce(Select<R::Target>) -> Select<R::Target>,
) -> Select<R::Target> {
    shape(<R::Target as Model>::query())
        .filter(relation.target_key().eq_outer(relation.owner_key()))
}

impl<M: Model> Select<M> {
    /// Rows with at least one related record: `User::query().has(User::POSTS)`.
    ///
    /// The related records keep their model's implicit scopes (soft
    /// deletes, tenant, default scope). A relation from a model to itself
    /// is not supported: the subquery can't tell the two tables apart.
    pub fn has<R: Relation<Owner = M>>(self, relation: R) -> Self {
        self.where_has(relation, |q| q)
    }

    /// Rows with at least one related record that `shape` keeps:
    ///
    /// ```ignore
    /// User::query().where_has(User::POSTS, |q| q.filter(Post::VIEWS.gte(100)))
    /// ```
    pub fn where_has<R: Relation<Owner = M>>(
        self,
        relation: R,
        shape: impl FnOnce(Select<R::Target>) -> Select<R::Target>,
    ) -> Self {
        self.filter(Expr::exists(related(relation, shape)))
    }

    /// Rows with no related record: `User::query().doesnt_have(User::POSTS)`.
    pub fn doesnt_have<R: Relation<Owner = M>>(self, relation: R) -> Self {
        self.where_doesnt_have(relation, |q| q)
    }

    /// Rows with no related record that `shape` keeps.
    pub fn where_doesnt_have<R: Relation<Owner = M>>(
        self,
        relation: R,
        shape: impl FnOnce(Select<R::Target>) -> Select<R::Target>,
    ) -> Self {
        self.filter(Expr::not_exists(related(relation, shape)))
    }

    /// Rows with a related record matching `condition`, the one-condition
    /// [`where_has`](Self::where_has):
    /// `User::query().where_relation(User::POSTS, Post::TITLE.eq("hello"))`.
    pub fn where_relation<R: Relation<Owner = M>>(
        self,
        relation: R,
        condition: Expr<R::Target>,
    ) -> Self {
        self.where_has(relation, |q| q.filter(condition))
    }
}

impl<M: Model> Select<M> {
    /// Loads how many records each row has through `relation`, beside the
    /// rows (one batched `GROUP BY` query); read it with
    /// [`Loaded::count`](crate::Loaded::count):
    ///
    /// ```ignore
    /// let view = User::query().with_count(User::POSTS).all(&db).await?;
    /// for user in view.rows() {
    ///     println!("{}: {}", user.name, view.count(User::POSTS, user).unwrap_or(0));
    /// }
    /// ```
    ///
    /// Other aggregates load the same way, through `with` and a handle:
    /// `.with(User::POSTS.sum::<i64>(Post::VIEWS))`.
    pub fn with_count<R: Relation<Owner = M>>(self, relation: R) -> WithSelect<M> {
        self.with(relation.count())
    }

    /// Loads whether each row has any record through `relation`; read it
    /// with [`Loaded::exists`](crate::Loaded::exists).
    pub fn with_exists<R: Relation<Owner = M>>(self, relation: R) -> WithSelect<M> {
        self.with(relation.exists())
    }
}
