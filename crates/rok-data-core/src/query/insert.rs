//! The insert builders.
use std::fmt;
use std::marker::PhantomData;

use crate::exec;
use crate::model::push_returning;
use crate::sql::Sql;
use crate::{Column, Error, Executor, Model, Result, Value};

use super::{Conflict, ConflictAction, ConflictTarget};

/// An `INSERT` of a single row built column by column, created with
/// [`Model::create`]. Columns you don't set get their database default;
/// timestamp columns default to `now()`.
pub struct Insert<M> {
    /// `(column, raw sql, params)`; plain values are stored as `"?"`.
    values: Vec<(&'static str, String, Vec<Value>)>,
    conflict_target: Option<ConflictTarget>,
    conflict_action: Option<ConflictAction>,
    _model: PhantomData<fn() -> M>,
}

impl<M: Model> Insert<M> {
    pub(crate) fn new() -> Self {
        Self {
            values: Vec::new(),
            conflict_target: None,
            conflict_action: None,
            _model: PhantomData,
        }
    }

    /// Set `column` to `value`.
    pub fn set(mut self, column: Column<M>, value: impl Into<Value>) -> Self {
        self.values
            .push((column.name(), "?".into(), vec![value.into()]));
        self
    }

    /// Set `column` to raw SQL, binding each `?` to the next parameter.
    pub fn set_raw<V: Into<Value>>(
        mut self,
        column: Column<M>,
        sql: impl Into<String>,
        params: impl IntoIterator<Item = V>,
    ) -> Self {
        let params = params.into_iter().map(Into::into).collect();
        self.values.push((column.name(), sql.into(), params));
        self
    }

    on_conflict_methods!();

    /// Render the `INSERT` statement. An incomplete `ON CONFLICT`
    /// configuration is left out here and reported by `exec`.
    pub fn to_sql(&self) -> Sql {
        self.build(false).expect("lenient rendering never fails")
    }

    fn build(&self, strict: bool) -> Result<Sql> {
        let mut values = self.values.clone();
        if let Some(column) = M::TENANT_COLUMN
            && let Some(tenant) = crate::model::tenant_override::<M>(column)
        {
            values.retain(|(c, _, _)| *c != column);
            values.push((column, "?".into(), vec![tenant]));
        }
        for ts in [M::CREATED_AT_COLUMN, M::UPDATED_AT_COLUMN]
            .into_iter()
            .flatten()
        {
            if !values.iter().any(|(c, _, _)| *c == ts) {
                values.push((ts, "now()".into(), Vec::new()));
            }
        }
        let mut sql = Sql::new();
        sql.push("INSERT INTO ").push_ident(M::TABLE);
        if values.is_empty() {
            sql.push(" DEFAULT VALUES");
        } else {
            sql.push(" (")
                .push_list(&values, ", ", |sql, (c, _, _)| {
                    sql.push_ident(c);
                })
                .push(") VALUES (")
                .push_list(&values, ", ", |sql, (_, raw, params)| {
                    sql.push_raw(raw, params);
                })
                .push(")");
        }
        let conflict = match self.conflict() {
            Ok(conflict) => conflict,
            Err(e) if strict => return Err(e),
            Err(_) => None,
        };
        if let Some(conflict) = conflict {
            let inserted: Vec<_> = values.iter().map(|(c, _, _)| *c).collect();
            let mut with_conflict = sql.clone();
            match conflict.write::<M>(&mut with_conflict, &inserted) {
                Ok(()) => sql = with_conflict,
                Err(e) if strict => return Err(e),
                Err(_) => {}
            }
        }
        push_returning::<M>(&mut sql);
        Ok(sql)
    }

    /// Run the insert and return the stored row. With
    /// [`do_nothing`](Self::do_nothing), a skipped row is an
    /// [`Error::NotFound`]; use [`exec_optional`](Self::exec_optional).
    pub async fn exec<'e, E: Executor<'e>>(self, executor: E) -> Result<M> {
        self.exec_optional(executor)
            .await?
            .ok_or_else(|| Error::not_found::<M>(None))
    }

    /// Run the insert and return the stored row, or `None` if it was
    /// skipped by `ON CONFLICT … DO NOTHING`.
    pub async fn exec_optional<'e, E: Executor<'e>>(self, executor: E) -> Result<Option<M>> {
        exec::fetch_optional(executor, &self.build(true)?, &[M::TABLE], false).await
    }
}

impl<M> Clone for Insert<M> {
    fn clone(&self) -> Self {
        Self {
            values: self.values.clone(),
            conflict_target: self.conflict_target.clone(),
            conflict_action: self.conflict_action.clone(),
            _model: PhantomData,
        }
    }
}

impl<M: Model> fmt::Debug for Insert<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Insert").field(&self.to_sql()).finish()
    }
}

/// An `INSERT` of whole records, created with [`Model::insert_many`].
/// Generated columns are skipped and timestamps are set to `now()`.
pub struct InsertMany<'a, M> {
    records: &'a [M],
    conflict_target: Option<ConflictTarget>,
    conflict_action: Option<ConflictAction>,
}

impl<'a, M: Model> InsertMany<'a, M> {
    pub(crate) fn new(records: &'a [M]) -> Self {
        Self {
            records,
            conflict_target: None,
            conflict_action: None,
        }
    }

    on_conflict_methods!();

    /// Render the `INSERT` statement. An incomplete `ON CONFLICT`
    /// configuration is left out here and reported by `exec`.
    pub fn to_sql(&self) -> Sql {
        let conflict = self.conflict().ok().flatten();
        crate::model::insert_sql(self.records, false, conflict.as_ref())
            .or_else(|_| crate::model::insert_sql(self.records, false, None))
            .expect("rendering without ON CONFLICT never fails")
    }

    fn build(&self) -> Result<Sql> {
        crate::model::insert_sql(self.records, false, self.conflict()?.as_ref())
    }

    /// Run the insert and return the stored rows. Rows skipped by
    /// `ON CONFLICT … DO NOTHING` are not returned.
    ///
    /// Every record is validated and passed to `before_insert` first; each
    /// returned row is passed to `after_insert`.
    pub async fn exec<'e, E: Executor<'e>>(self, executor: E) -> Result<Vec<M>> {
        if self.records.is_empty() {
            return Ok(Vec::new());
        }
        for record in self.records {
            crate::model::pre_insert(record)?;
        }
        let rows: Vec<M> = exec::fetch_all(executor, &self.build()?, &[M::TABLE], false).await?;
        for row in &rows {
            row.after_insert()?;
        }
        Ok(rows)
    }
}

impl<M> Clone for InsertMany<'_, M> {
    fn clone(&self) -> Self {
        Self {
            records: self.records,
            conflict_target: self.conflict_target.clone(),
            conflict_action: self.conflict_action.clone(),
        }
    }
}

impl<M: Model> fmt::Debug for InsertMany<'_, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("InsertMany").field(&self.to_sql()).finish()
    }
}
