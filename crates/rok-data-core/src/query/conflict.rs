//! The `ON CONFLICT` handling of an insert.
use crate::sql::Sql;
use crate::{Error, Model, Result};

pub(crate) fn quoted(ident: &str) -> String {
    let mut s = String::new();
    crate::sql::push_ident(&mut s, ident);
    s
}

// ----- ON CONFLICT -------------------------------------------------------------

#[derive(Debug, Clone)]
pub(crate) enum ConflictTarget {
    None,
    Columns(Vec<&'static str>),
    Constraint(&'static str),
}

#[derive(Debug, Clone)]
pub(crate) enum ConflictAction {
    Nothing,
    Update(Vec<&'static str>),
    UpdateAll,
}

/// An `ON CONFLICT` clause.
#[derive(Debug, Clone)]
pub(crate) struct Conflict {
    pub(crate) target: ConflictTarget,
    pub(crate) action: ConflictAction,
}

impl Conflict {
    /// Write ` ON CONFLICT … DO …` for an insert of `inserted` columns.
    pub(crate) fn write<M: Model>(&self, sql: &mut Sql, inserted: &[&'static str]) -> Result<()> {
        sql.push(" ON CONFLICT");
        let target_columns: &[&str] = match &self.target {
            ConflictTarget::None => &[],
            ConflictTarget::Columns(columns) => {
                sql.push(" (")
                    .push_list(columns, ", ", |sql, c| {
                        sql.push_ident(c);
                    })
                    .push(")");
                columns
            }
            ConflictTarget::Constraint(name) => {
                sql.push(" ON CONSTRAINT ").push_ident(name);
                &[]
            }
        };
        let mut updates: Vec<&'static str> = match &self.action {
            ConflictAction::Nothing => {
                sql.push(" DO NOTHING");
                return Ok(());
            }
            ConflictAction::Update(columns) => columns.clone(),
            ConflictAction::UpdateAll => inserted
                .iter()
                .copied()
                .filter(|c| {
                    !target_columns.contains(c)
                        && !M::PRIMARY_KEYS.contains(c)
                        && M::CREATED_AT_COLUMN != Some(*c)
                })
                .collect(),
        };
        if matches!(self.target, ConflictTarget::None) {
            return Err(Error::InvalidQuery(
                "ON CONFLICT DO UPDATE needs conflict columns or a constraint".into(),
            ));
        }
        // Managed columns are maintained, never copied from the new row; the
        // tenant of an existing row never changes.
        updates.retain(|c| {
            M::VERSION_COLUMN != Some(*c)
                && M::UPDATED_AT_COLUMN != Some(*c)
                && M::TENANT_COLUMN != Some(*c)
        });
        sql.push(" DO UPDATE SET ");
        let mut first = true;
        let mut sep = |sql: &mut Sql| {
            if !std::mem::take(&mut first) {
                sql.push(", ");
            }
        };
        for column in &updates {
            sep(sql);
            sql.push_ident(column)
                .push(" = EXCLUDED.")
                .push_ident(column);
        }
        if let Some(updated_at) = M::UPDATED_AT_COLUMN {
            sep(sql);
            sql.push_ident(updated_at).push(" = now()");
        }
        if let Some(version) = M::VERSION_COLUMN {
            sep(sql);
            sql.push_ident(version)
                .push(" = ")
                .push_ident(M::TABLE)
                .push(".")
                .push_ident(version)
                .push(" + 1");
        }
        if first {
            // Nothing to change, but `DO UPDATE` still returns the row.
            sql.push_ident(M::PRIMARY_KEY)
                .push(" = ")
                .push_ident(M::TABLE)
                .push(".")
                .push_ident(M::PRIMARY_KEY);
        }
        // Never update another tenant's row: the conflicting row must belong
        // to the tenant being written (no row is returned otherwise).
        if let Some(tenant) = M::TENANT_COLUMN
            && inserted.contains(&tenant)
        {
            sql.push(" WHERE ")
                .push_ident(M::TABLE)
                .push(".")
                .push_ident(tenant)
                .push(" = EXCLUDED.")
                .push_ident(tenant);
        }
        Ok(())
    }
}
