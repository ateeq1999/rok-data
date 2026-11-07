//! Statements, CTEs, and the tables each brings into scope.

use std::rc::Rc;

use sqlparser::ast::{
    AssignmentTarget, ConflictTarget, Cte, Expr, FromTable, JoinConstraint, JoinOperator,
    LimitClause, OnConflictAction, OnInsert, Query, Select, SelectItem, SetExpr, SetOperator,
    Statement, TableFactor, TableObject, TableWithJoins, UpdateTableFromKind,
};

use super::conflict::{check_conflict_constraint, check_conflict_target};
use super::{Analysis, Output, ScopeEntry, TableRef};
use crate::parser::naming;
use crate::parser::postgres::{ident_name, object_leaf_name};
use crate::schema::{Column, Shape, Table};

impl<'a, 'c> Analysis<'a, 'c> {
    pub(super) fn statement(&mut self, statement: &Statement) -> Result<Vec<Output>, String> {
        match statement {
            Statement::Query(query) => self.query(query),
            Statement::Insert(insert) => {
                let TableObject::TableName(name) = &insert.table else {
                    return Err("INSERT into a table function is not supported".to_owned());
                };
                let table = self.resolve_table(name)?;
                let qualifier = insert
                    .table_alias
                    .as_ref()
                    .map(ident_name)
                    .unwrap_or_else(|| table.sql_name.clone());
                self.push_scope(qualifier, TableRef::Schema(table), false);

                let columns: Vec<&Column> = if insert.columns.is_empty() {
                    table.columns.iter().collect()
                } else {
                    insert
                        .columns
                        .iter()
                        .map(|ident| self.table_column(table, &ident_name(ident)))
                        .collect::<Result<_, _>>()?
                };
                let Some(source) = &insert.source else {
                    return Err("INSERT without VALUES or SELECT is not supported".to_owned());
                };
                match source.body.as_ref() {
                    SetExpr::Values(values) => {
                        for row in &values.rows {
                            if row.len() != columns.len() {
                                return Err(format!(
                                    "a VALUES row has {} values for {} columns",
                                    row.len(),
                                    columns.len()
                                ));
                            }
                            for (column, value) in columns.iter().zip(row) {
                                self.typed_by_column(value, column, column.nullable, "")?;
                            }
                        }
                    }
                    _ => self.insert_select(source, &columns)?,
                }

                if let Some(OnInsert::OnConflict(on_conflict)) = &insert.on {
                    match &on_conflict.conflict_target {
                        Some(ConflictTarget::Columns(target)) => {
                            check_conflict_target(table, target, self.conflict_predicate.as_ref())?;
                        }
                        Some(ConflictTarget::OnConstraint(name)) => {
                            check_conflict_constraint(table, name)?;
                        }
                        None => {}
                    }
                }
                if let Some(OnInsert::OnConflict(on_conflict)) = &insert.on
                    && let OnConflictAction::DoUpdate(update) = &on_conflict.action
                {
                    self.push_scope("excluded".to_owned(), TableRef::Schema(table), false);
                    for assignment in &update.assignments {
                        self.assignment(table, &assignment.target, &assignment.value)?;
                    }
                    if let Some(selection) = &update.selection {
                        self.expr(selection)?;
                    }
                    self.scope.pop();
                }
                self.returning(insert.returning.as_deref())
            }
            Statement::Update {
                table,
                assignments,
                from,
                selection,
                returning,
                ..
            } => {
                if !table.joins.is_empty() {
                    return Err("UPDATE with joins is not supported; join in `FROM`".to_owned());
                }
                let first = self.scope.len();
                self.scope_table(table)?;
                let TableRef::Schema(target) = self.scope[first].table else {
                    return Err("UPDATE must target a schema table, not a CTE".to_owned());
                };
                // `FROM` tables join the target: their columns resolve
                // beside its own, in the `SET` values, `WHERE` and
                // `RETURNING`.
                if let Some(
                    UpdateTableFromKind::AfterSet(from) | UpdateTableFromKind::BeforeSet(from),
                ) = from
                {
                    for table in from {
                        self.scope_table(table)?;
                    }
                }
                for assignment in assignments {
                    self.assignment(target, &assignment.target, &assignment.value)?;
                }
                if let Some(selection) = selection {
                    self.expr(selection)?;
                }
                self.returning(returning.as_deref())
            }
            Statement::Delete(delete) => {
                let (FromTable::WithFromKeyword(from) | FromTable::WithoutKeyword(from)) =
                    &delete.from;
                let [table] = from.as_slice() else {
                    return Err(
                        "DELETE must name exactly one table; name the others in `USING`".to_owned(),
                    );
                };
                if !table.joins.is_empty() {
                    return Err("DELETE with joins is not supported; join in `USING`".to_owned());
                }
                self.scope_table(table)?;
                // `USING` tables join the target, as `UPDATE … FROM`'s do.
                for table in delete.using.iter().flatten() {
                    self.scope_table(table)?;
                }
                if let Some(selection) = &delete.selection {
                    self.expr(selection)?;
                }
                self.returning(delete.returning.as_deref())
            }
            _ => Err("only SELECT, INSERT, UPDATE, and DELETE statements are supported".to_owned()),
        }
    }

    /// Analyzes a subquery (scalar, `EXISTS` or `IN`): its own tables come
    /// first, and it sees the tables of the queries around it, so a
    /// correlated column resolves. Its placeholders are typed, and the type
    /// of its one column is recorded for the result typing.
    ///
    /// A subquery the engine can't analyze (a form it doesn't support) is
    /// left untyped rather than failing the query, as subqueries were before
    /// they were analyzed: its placeholders still need their type from
    /// elsewhere, and a result column drawn from it still needs a
    /// `-- column:` override.
    pub(super) fn subquery(&mut self, query: &Query) {
        let key = std::ptr::from_ref(query);
        if self.subquery_types.contains_key(&key) {
            return;
        }
        let depth = self.scope.len();
        self.levels.push(depth);
        let outputs = self.query(query);
        self.levels.pop();
        self.scope.truncate(depth);
        let value_type = match outputs.as_deref() {
            Ok([output]) => {
                output
                    .rust_type
                    .as_deref()
                    .map(|rust_type| match Shape::of(rust_type) {
                        Shape::Optional(inner) => inner.to_owned(),
                        Shape::List(_) | Shape::Value(_) => rust_type.to_owned(),
                    })
            }
            _ => None,
        };
        self.subquery_types.insert(key, value_type);
    }

    fn query(&mut self, query: &Query) -> Result<Vec<Output>, String> {
        let visible = self.ctes.len();
        self.query_depth += 1;
        let outputs = self.query_body(query);
        self.query_depth -= 1;
        self.ctes.truncate(visible);
        outputs
    }

    /// `INSERT … SELECT`: each value the `SELECT` returns is typed from the
    /// column it fills, as a `VALUES` row's is. The `SELECT` runs in its own
    /// scope, without the target: its `FROM` names the tables it reads,
    /// the target included. Each branch of a set operation fills the
    /// columns the same way.
    fn insert_select(&mut self, source: &Query, columns: &[&Column]) -> Result<(), String> {
        self.in_own_scope(|analysis| analysis.insert_query(source, columns))
    }

    fn insert_query(&mut self, query: &Query, columns: &[&Column]) -> Result<(), String> {
        let visible = self.ctes.len();
        let typed = self
            .with_clause(query)
            .and_then(|()| self.insert_body(&query.body, columns))
            .and_then(|()| self.limit_clause(query));
        self.ctes.truncate(visible);
        typed
    }

    fn insert_body(&mut self, body: &SetExpr, columns: &[&Column]) -> Result<(), String> {
        match body {
            SetExpr::Select(select) => self.insert_select_values(select, columns),
            SetExpr::SetOperation { left, right, .. } => {
                self.in_level(|analysis| analysis.insert_body(left, columns))?;
                self.in_level(|analysis| analysis.insert_body(right, columns))
            }
            SetExpr::Query(query) => {
                self.in_level(|analysis| analysis.insert_query(query, columns))
            }
            _ => Err(
                "INSERT takes VALUES, a SELECT, or SELECTs joined by UNION, INTERSECT or EXCEPT"
                    .to_owned(),
            ),
        }
    }

    fn insert_select_values(&mut self, select: &Select, columns: &[&Column]) -> Result<(), String> {
        let values = select
            .projection
            .iter()
            .map(|item| match item {
                SelectItem::UnnamedExpr(expr) | SelectItem::ExprWithAlias { expr, .. } => Ok(expr),
                SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(..) => Err(
                    "INSERT … SELECT * is not supported; select each value the columns take"
                        .to_owned(),
                ),
            })
            .collect::<Result<Vec<&Expr>, String>>()?;
        if values.len() != columns.len() {
            return Err(format!(
                "the SELECT returns {} values for {} columns",
                values.len(),
                columns.len()
            ));
        }
        for table in &select.from {
            self.scope_table(table)?;
        }
        for (column, value) in columns.iter().zip(values) {
            self.typed_by_column(value, column, column.nullable, "")?;
        }
        if let Some(selection) = &select.selection {
            self.expr(selection)?;
        }
        if let Some(having) = &select.having {
            self.expr(having)?;
        }
        Ok(())
    }

    /// Runs `analyze` on a query beside the current one (a set operation's
    /// branch): it sees the queries around it, and its own tables are
    /// gone after.
    fn in_level<T>(&mut self, analyze: impl FnOnce(&mut Self) -> T) -> T {
        let depth = self.scope.len();
        self.levels.push(depth);
        let result = analyze(self);
        self.levels.pop();
        self.scope.truncate(depth);
        result
    }

    /// A set operation's result columns: named by its left branch, each of
    /// the type its branches share (one type, a typed id's plain type, or
    /// the widest number, as `CASE`'s branches share theirs).
    fn set_operation(
        &mut self,
        op: SetOperator,
        left: &SetExpr,
        right: &SetExpr,
    ) -> Result<Vec<Output>, String> {
        let left = self.in_level(|analysis| analysis.branch(left))?;
        let right = self.in_level(|analysis| analysis.branch(right))?;
        if left.len() != right.len() {
            let unmatched = left
                .get(right.len())
                .or_else(|| right.get(left.len()))
                .map_or("", |output| output.sql_name.as_str());
            return Err(format!(
                "the branches of {op} return {} and {} columns: column `{unmatched}` has no match",
                left.len(),
                right.len()
            ));
        }
        left.into_iter()
            .zip(right)
            .map(|(left, right)| self.set_output(op, left, right))
            .collect()
    }

    fn branch(&mut self, body: &SetExpr) -> Result<Vec<Output>, String> {
        match body {
            SetExpr::Select(select) => self.select(select),
            SetExpr::Query(query) => self.query(query),
            SetExpr::SetOperation {
                op, left, right, ..
            } => self.set_operation(*op, left, right),
            _ => Err(
                "each branch of UNION, INTERSECT or EXCEPT must be a SELECT (not VALUES)"
                    .to_owned(),
            ),
        }
    }

    /// Analyzes the query's CTEs, making each visible to those after it and
    /// to the query's body.
    fn with_clause(&mut self, query: &Query) -> Result<(), String> {
        if let Some(with) = &query.with {
            for cte in &with.cte_tables {
                match cte.query.body.as_ref() {
                    SetExpr::SetOperation {
                        op: SetOperator::Union,
                        left,
                        right,
                        ..
                    } if with.recursive => self.recursive_cte(cte, left, right)?,
                    _ => self.cte(cte)?,
                }
            }
        }
        Ok(())
    }

    fn limit_clause(&mut self, query: &Query) -> Result<(), String> {
        match &query.limit_clause {
            Some(LimitClause::LimitOffset { limit, offset, .. }) => {
                if let Some(limit) = limit {
                    self.typed_as(limit, "limit", "i64")?;
                }
                if let Some(offset) = offset {
                    self.typed_as(&offset.value, "offset", "i64")?;
                }
                Ok(())
            }
            Some(LimitClause::OffsetCommaLimit { .. }) => {
                Err("`LIMIT offset, count` is MySQL syntax; use LIMIT … OFFSET …".to_owned())
            }
            None => Ok(()),
        }
    }

    fn query_body(&mut self, query: &Query) -> Result<Vec<Output>, String> {
        self.with_clause(query)?;
        let outputs = match query.body.as_ref() {
            SetExpr::Select(select) => self.select(select)?,
            // `ORDER BY` and `LIMIT` apply to the whole result.
            SetExpr::SetOperation {
                op, left, right, ..
            } => self.set_operation(*op, left, right)?,
            // The statement after a `WITH`, or a data-modifying CTE's own:
            // its `RETURNING` gives the result.
            SetExpr::Insert(statement)
            | SetExpr::Update(statement)
            | SetExpr::Delete(statement) => {
                return self.statement(statement);
            }
            _ => {
                return Err(
                    "a query must be a SELECT, a set operation of SELECTs, or (after WITH) an INSERT, UPDATE or DELETE".to_owned(),
                );
            }
        };
        self.limit_clause(query)?;
        Ok(outputs)
    }

    /// Analyzes a CTE's query in its own scope and makes its result columns
    /// visible, under the CTE's name, to later CTEs and the main statement.
    ///
    /// A data-modifying CTE (`INSERT`, `UPDATE` or `DELETE`) makes its
    /// `RETURNING` columns visible the same way. PostgreSQL allows one only
    /// in the statement's own `WITH`.
    fn cte(&mut self, cte: &Cte) -> Result<(), String> {
        let name = ident_name(&cte.alias.name);
        match cte.query.body.as_ref() {
            SetExpr::Merge(_) => {
                return Err(format!("MERGE in CTE `{name}` is not supported"));
            }
            SetExpr::Insert(_) | SetExpr::Update(_) | SetExpr::Delete(_)
                if self.query_depth != 1 =>
            {
                return Err(format!(
                    "data-modifying CTE `{name}` must be in the statement's own WITH, not nested in a CTE or subquery"
                ));
            }
            _ => {}
        }
        let outputs = self.in_own_scope(|analysis| analysis.query(&cte.query));
        let table = self.cte_table(cte, outputs?)?;
        self.ctes.push(Rc::new(table));
        Ok(())
    }

    /// Analyzes a recursive CTE, `anchor UNION [ALL] recursive-term`: the
    /// anchor's result columns give the CTE's, and the recursive term, which
    /// sees the CTE, must return as many columns of the same types. A
    /// nullable column of either term makes the CTE's column nullable.
    fn recursive_cte(&mut self, cte: &Cte, anchor: &SetExpr, term: &SetExpr) -> Result<(), String> {
        let name = ident_name(&cte.alias.name);
        let anchor = self.term(anchor)?;
        let mut table = self.cte_table(cte, anchor)?;
        self.ctes.push(Rc::new(table.clone()));
        let outputs = self.term(term);
        self.ctes.pop();
        let outputs = outputs?;
        if outputs.len() != table.columns.len() {
            return Err(format!(
                "the recursive term of CTE `{name}` returns {} columns but its anchor returns {}",
                outputs.len(),
                table.columns.len()
            ));
        }
        for (column, output) in table.columns.iter_mut().zip(outputs) {
            let Some(rust_type) = output.rust_type else {
                continue;
            };
            let (base, nullable) = match Shape::of(&rust_type) {
                Shape::Optional(inner) => (inner, true),
                Shape::List(_) | Shape::Value(_) => (rust_type.as_str(), false),
            };
            if base != column.sqlx_type {
                return Err(format!(
                    "the recursive term of CTE `{name}` returns `{base}` for column `{}`, but its anchor returns `{}`; cast one of them",
                    column.sql_name, column.sqlx_type
                ));
            }
            if nullable && !column.nullable {
                column.nullable = true;
                column.rust_type = format!("Option<{}>", column.sqlx_type);
            }
        }
        self.ctes.push(Rc::new(table));
        Ok(())
    }

    /// One term of a set operation, analyzed in its own scope.
    fn term(&mut self, body: &SetExpr) -> Result<Vec<Output>, String> {
        self.in_own_scope(|analysis| match body {
            SetExpr::Select(select) => analysis.select(select),
            SetExpr::Query(query) => analysis.query(query),
            _ => Err(
                "a recursive CTE's terms must each be a plain SELECT (no nested UNION or VALUES)"
                    .to_owned(),
            ),
        })
    }

    /// The table a CTE's result columns make, named by its column list or
    /// after its query's columns.
    fn cte_table(&self, cte: &Cte, outputs: Vec<Output>) -> Result<Table, String> {
        let name = ident_name(&cte.alias.name);
        let aliases = &cte.alias.columns;
        if aliases.len() > outputs.len() {
            return Err(format!(
                "CTE `{name}` names {} columns but its query returns {}",
                aliases.len(),
                outputs.len()
            ));
        }
        let mut columns: Vec<Column> = Vec::with_capacity(outputs.len());
        for (index, output) in outputs.into_iter().enumerate() {
            let sql_name = match (aliases.get(index), output.name_error) {
                (Some(alias), _) => ident_name(&alias.name),
                (None, Some(name_error)) => return Err(name_error),
                (None, None) => output.sql_name,
            };
            if columns.iter().any(|column| column.sql_name == sql_name) {
                return Err(format!(
                    "CTE `{name}` returns column `{sql_name}` more than once; alias the duplicates"
                ));
            }
            let Some(rust_type) = output.rust_type else {
                return Err(format!(
                    "cannot infer the type of column `{sql_name}` of CTE `{name}`; cast it explicitly (`(expr)::type AS {sql_name}`)"
                ));
            };
            let (sqlx_type, nullable) = match Shape::of(&rust_type) {
                Shape::Optional(inner) => (inner.to_owned(), true),
                Shape::List(_) | Shape::Value(_) => (rust_type.clone(), false),
            };
            columns.push(Column {
                rust_name: naming::rust_identifier(&sql_name)?,
                sql_name,
                // CTE columns exist only within their query; no SQL type is
                // declared for them.
                sql_type: String::new(),
                rust_type,
                sqlx_type,
                nullable,
                is_primary_key: false,
                has_default: false,
                default: None,
                generated: None,
                identity: None,
                is_generated: false,
                value_list: None,
                declared_length: None,
                value_range: None,
                length_range: None,
                typed_id: None,
                is_managed: false,
                id_generation: None,
            });
        }
        Ok(Table {
            schema: None,
            module_name: String::new(),
            struct_name: String::new(),
            sql_name: name,
            columns,
            primary_key: Vec::new(),
            foreign_keys: Vec::new(),
            unique_keys: Vec::new(),
            key_constraints: Vec::new(),
            named_constraints: Vec::new(),
            version_column: None,
            soft_delete_column: None,
            tenant_column: None,
            relationships: Vec::new(),
            scopes: Vec::new(),
            before_insert_triggers: Vec::new(),
        })
    }

    fn select(&mut self, select: &Select) -> Result<Vec<Output>, String> {
        for table in &select.from {
            self.scope_table(table)?;
        }
        if let Some(selection) = &select.selection {
            self.expr(selection)?;
        }
        if let Some(having) = &select.having {
            self.expr(having)?;
        }
        self.projection(&select.projection)
    }

    fn scope_table(&mut self, table: &TableWithJoins) -> Result<(), String> {
        // A `RIGHT`/`FULL` join makes nullable only the tables joined before
        // it in this item, not those of other `FROM` items (or an
        // `UPDATE`'s target).
        let first = self.scope.len();
        self.table_factor(&table.relation, false)?;
        for join in &table.joins {
            let (constraint, left_nullable, right_nullable) = match &join.join_operator {
                JoinOperator::Join(constraint) | JoinOperator::Inner(constraint) => {
                    (constraint, false, false)
                }
                JoinOperator::Left(constraint) | JoinOperator::LeftOuter(constraint) => {
                    (constraint, false, true)
                }
                JoinOperator::Right(constraint) | JoinOperator::RightOuter(constraint) => {
                    (constraint, true, false)
                }
                JoinOperator::FullOuter(constraint) => (constraint, true, true),
                JoinOperator::CrossJoin(constraint) => (constraint, false, false),
                other => return Err(format!("join `{other:?}` is not supported")),
            };
            if left_nullable {
                for entry in &mut self.scope[first..] {
                    entry.outer_nullable = true;
                }
            }
            self.table_factor(&join.relation, right_nullable)?;
            match constraint {
                JoinConstraint::On(expr) => self.expr(expr)?,
                JoinConstraint::None => {}
                JoinConstraint::Using(_) | JoinConstraint::Natural => {
                    return Err(
                        "JOIN … USING and NATURAL JOIN are not supported; use ON".to_owned()
                    );
                }
            }
        }
        Ok(())
    }

    fn table_factor(&mut self, factor: &TableFactor, outer_nullable: bool) -> Result<(), String> {
        let (qualifier, table) = match factor {
            TableFactor::Table {
                name,
                alias,
                args: None,
                ..
            } => {
                let table = self.resolve_from(name)?;
                let qualifier = alias
                    .as_ref()
                    .map(|alias| ident_name(&alias.name))
                    .unwrap_or_else(|| table.sql_name.clone());
                (qualifier, table)
            }
            _ => match self.set_returning_table(factor) {
                Some(table) => {
                    let table = table?;
                    (table.sql_name.clone(), TableRef::Cte(Rc::new(table)))
                }
                None => {
                    return Err(
                        "only tables and set-returning functions are supported in FROM (no subqueries)"
                            .to_owned(),
                    );
                }
            },
        };
        // A subquery's table may take a name an outer query's has: it
        // shadows that one.
        if self.scope[self.floor()..]
            .iter()
            .any(|entry| entry.qualifier == qualifier)
        {
            return Err(format!(
                "`{qualifier}` appears twice in FROM; alias one of them"
            ));
        }
        self.push_scope(qualifier, table, outer_nullable);
        Ok(())
    }

    fn push_scope(&mut self, qualifier: String, table: TableRef<'a>, outer_nullable: bool) {
        self.scope.push(ScopeEntry {
            qualifier,
            table,
            outer_nullable,
        });
    }

    fn assignment(
        &mut self,
        table: &Table,
        target: &AssignmentTarget,
        value: &Expr,
    ) -> Result<(), String> {
        let AssignmentTarget::ColumnName(name) = target else {
            return Err("tuple assignments `SET (a, b) = …` are not supported".to_owned());
        };
        let column = self.table_column(table, &object_leaf_name(name))?;
        self.typed_by_column(value, column, column.nullable, "")
    }

    fn returning(&mut self, returning: Option<&[SelectItem]>) -> Result<Vec<Output>, String> {
        match returning {
            Some(items) => self.projection(items),
            None => Ok(Vec::new()),
        }
    }
}
