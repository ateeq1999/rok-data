//! The query builders: the reads, the writes, and their terminals.

use crate::expr::Cond;
use crate::model::Model;
use crate::sql::Sql;
use crate::{Column, Expr};

/// One leg of a `with`-loaded query on the session's [`Journey`]: every
/// arm re-borrows the path, so the rows and each include's batched load
/// share one pool — or run inside one transaction.
macro_rules! on_journey {
    ($journey:expr, |$e:ident| $run:expr) => {
        match &mut $journey {
            $crate::exec::Journey::Database(db) => {
                let $e = &**db;
                $run
            }
            $crate::exec::Journey::Transaction(transaction) => {
                let $e = &mut **transaction;
                $run
            }
            $crate::exec::Journey::Pool(pool) => {
                let $e = &**pool;
                $run
            }
            $crate::exec::Journey::Connection(connection) => {
                let $e = &mut **connection;
                $run
            }
        }
    };
}

fn push_where<'a>(sql: &mut Sql, keyword: &str, filters: impl IntoIterator<Item = &'a Cond>) {
    let mut filters = filters.into_iter().peekable();
    if filters.peek().is_none() {
        return;
    }
    sql.push(keyword);
    // Groups render their own parentheses, so a top-level AND chain is safe.
    sql.push_list(filters, " AND ", |sql, cond| cond.write(sql));
}

/// Which soft-deleted rows a query sees (only relevant for models with a
/// `deleted_at` column).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Trashed {
    Exclude,
    Include,
    Only,
}

impl Trashed {
    /// The implicit soft-delete condition for model `M`, if any.
    fn cond<M: Model>(self) -> Option<Cond> {
        let column = M::DELETED_AT_COLUMN?;
        match self {
            Trashed::Exclude => Some(Column::<M>::new(column).is_null().cond),
            Trashed::Only => Some(Column::<M>::new(column).is_not_null().cond),
            Trashed::Include => None,
        }
    }
}

/// Implicit conditions: the tenant, the soft-delete scope and, unless
/// `unscoped`, the model's default scope.
fn implicit<M: Model>(trashed: Trashed, scoped: bool, all_tenants: bool) -> Vec<Cond> {
    let default = scoped.then(M::default_scope).flatten().map(|e| e.cond);
    tenant_cond::<M>(all_tenants)
        .into_iter()
        .chain(trashed.cond::<M>())
        .chain(default)
        .collect()
}

/// `tenant = current` inside a tenant scope; `FALSE` outside one (fail
/// closed) unless `all_tenants`.
fn tenant_cond<M: Model>(all_tenants: bool) -> Option<Cond> {
    let column = M::TENANT_COLUMN.filter(|_| !all_tenants)?;
    Some(match crate::tenant::current() {
        Some(tenant) => Column::<M>::new(column).eq(tenant).cond,
        None => {
            tracing::warn!(
                target: "rok_data::tenant",
                table = M::TABLE,
                "query on a tenant model outside `with_tenant`; matching no rows (use `all_tenants()` to opt out)"
            );
            Expr::<M>::none().cond
        }
    })
}
macro_rules! on_conflict_methods {
    () => {
        /// Handle conflicts on these columns (a unique index or the primary
        /// key). Follow with [`do_nothing`](Self::do_nothing),
        /// [`do_update`](Self::do_update) or
        /// [`do_update_all`](Self::do_update_all).
        pub fn on_conflict(mut self, columns: impl IntoIterator<Item = Column<M>>) -> Self {
            self.conflict_target = Some(ConflictTarget::Columns(
                columns.into_iter().map(|c| c.name()).collect(),
            ));
            self
        }

        /// Handle conflicts on the named constraint.
        pub fn on_constraint(mut self, name: &'static str) -> Self {
            self.conflict_target = Some(ConflictTarget::Constraint(name));
            self
        }

        /// `ON CONFLICT … DO NOTHING`: skip conflicting rows. Without
        /// [`on_conflict`](Self::on_conflict), any conflict is ignored.
        pub fn do_nothing(mut self) -> Self {
            self.conflict_action = Some(ConflictAction::Nothing);
            self
        }

        /// `ON CONFLICT … DO UPDATE` setting these columns from the new row.
        pub fn do_update(mut self, columns: impl IntoIterator<Item = Column<M>>) -> Self {
            self.conflict_action = Some(ConflictAction::Update(
                columns.into_iter().map(|c| c.name()).collect(),
            ));
            self
        }

        /// `ON CONFLICT … DO UPDATE` setting every inserted column except the
        /// conflict columns, the primary key and `created_at`. With
        /// [`on_constraint`](Self::on_constraint) the constraint's columns
        /// aren't known, so they are rewritten with their (equal) new values.
        pub fn do_update_all(mut self) -> Self {
            self.conflict_action = Some(ConflictAction::UpdateAll);
            self
        }

        fn conflict(&self) -> Result<Option<Conflict>> {
            match (&self.conflict_target, &self.conflict_action) {
                (None, None) => Ok(None),
                (target, Some(action)) => Ok(Some(Conflict {
                    target: target.clone().unwrap_or(ConflictTarget::None),
                    action: action.clone(),
                })),
                (Some(_), None) => Err(Error::InvalidQuery(
                    "`on_conflict` needs `do_nothing`, `do_update` or `do_update_all`".into(),
                )),
            }
        }
    };
}

mod conflict;
mod insert;
mod memoized;
mod projected;
mod select;
mod update;
mod with;

pub(crate) use conflict::{Conflict, ConflictAction, ConflictTarget};
pub use insert::{Insert, InsertMany};
pub use memoized::Memoized;
pub use projected::Projected;
pub use select::{__cursor_sql, __paginate_sql, Select};
pub(crate) use select::{JoinClause, join_scope};
pub use update::Update;
pub use with::WithSelect;
