//! Keyset ("cursor") pagination: [`Select::cursor_paginate`], forward and
//! back, over plain column orders and — for joins and expression orders —
//! over a wrapped subquery of aliased sort keys.

use crate::cursor::{Cursor, CursorPage};
use crate::exec;
use crate::expr::{Cond, Direction};
use crate::model::{Model, push_columns};
use crate::sql::Sql;
use crate::{Column, Error, Executor, Expr, Order, Result, Value};

use super::Select;

impl<M: Model> Select<M> {
    /// The ordering used for keyset pagination: the query's `ORDER BY`
    /// columns plus the primary key as a tiebreaker.
    pub(crate) fn keyset_order(&self) -> Vec<(&'static str, Direction)> {
        let mut order: Vec<_> = self
            .order
            .iter()
            .filter_map(|o| o.column().map(|c| (c, o.direction)))
            .collect();
        for key in M::PRIMARY_KEYS {
            if !order.iter().any(|(c, _)| c == key) {
                order.push((key, Direction::Asc));
            }
        }
        order
    }

    /// Keyset ("cursor") pagination: up to `per_page` rows on from
    /// `cursor`, following the query's `ORDER BY` (the primary key is added
    /// as a tiebreaker). Unlike [`paginate`](Self::paginate), the cost
    /// doesn't grow with the page number and rows aren't skipped or repeated
    /// when data changes between requests.
    ///
    /// Pass `None` for the first page, then either of the page's cursors:
    /// `page.next` continues forward, `page.prev` goes back — the rows come
    /// in the query's order either way. Sort keys (columns or expressions)
    /// must be `NOT NULL`; expression orders and joined queries paginate
    /// over a wrapped subquery.
    ///
    /// ```ignore
    /// let page = Post::order_by(Post::CREATED_AT.desc())
    ///     .cursor_paginate(&db, None, 20)
    ///     .await?;
    /// let next = page.next.map(|c| c.to_string()); // hand to the client
    ///
    /// // A client token parses back (a bad one is `Error::InvalidCursor`).
    /// let cursor: Cursor = next.unwrap().parse()?;
    /// let page2 = Post::order_by(Post::CREATED_AT.desc())
    ///     .cursor_paginate(&db, Some(&cursor), 20)
    ///     .await?;
    /// let back = Post::order_by(Post::CREATED_AT.desc())
    ///     .cursor_paginate(&db, page2.prev.as_ref(), 20)
    ///     .await?; // the first page again
    /// ```
    pub async fn cursor_paginate<'e, E: Executor<'e>>(
        self,
        executor: E,
        cursor: Option<&Cursor>,
        per_page: u64,
    ) -> Result<CursorPage<M>> {
        if per_page == 0 {
            return Err(Error::InvalidQuery(
                "`per_page` must be greater than 0".into(),
            ));
        }
        if self.needs_wrapped_keyset() {
            return self
                .wrapped_cursor_paginate(executor, cursor, per_page)
                .await;
        }
        let select = self.cursor_select(cursor, per_page)?;
        let order = select.keyset_order();
        let mut items = select.all(executor).await?;
        let more = items.len() as u64 > per_page;
        items.truncate(per_page as usize);
        if cursor.is_some_and(Cursor::is_backward) {
            items.reverse();
        }
        let values = |row: &M| {
            order
                .iter()
                .map(|(c, _)| {
                    row.value_of(c).ok_or_else(|| {
                        Error::InvalidQuery(format!("`{c}` is not a column of `{}`", M::TABLE))
                    })
                })
                .collect::<Result<Vec<_>>>()
        };
        let edges = Edges::of(cursor, more);
        let next = match items.last() {
            Some(last) if edges.next => Some(Cursor::new(values(last)?)),
            _ => None,
        };
        let prev = match items.first() {
            Some(first) if edges.prev => Some(Cursor::before(values(first)?)),
            _ => None,
        };
        Ok(CursorPage { items, next, prev })
    }

    /// Joined queries and expression orders paginate over aliased sort keys.
    pub(crate) fn needs_wrapped_keyset(&self) -> bool {
        !self.joins.is_empty() || self.order.iter().any(|o| o.column().is_none())
    }

    /// The order with the root primary key appended as a tiebreaker.
    pub(crate) fn keyset_orders(&self) -> Vec<Order<M>> {
        let mut order = self.order.clone();
        for key in M::PRIMARY_KEYS {
            let present = order.iter().any(|o| {
                matches!(o.target, crate::expr::OrderTarget::Column { table, name } if table == M::TABLE && name == *key)
            });
            if !present {
                order.push(Column::<M>::new(key).asc());
            }
        }
        order
    }

    /// ```sql
    /// SELECT * FROM (
    ///     SELECT [DISTINCT ON (pk)] <columns>, <sort keys> AS __rok_o{i} FROM … WHERE …
    /// ) k WHERE (k.__rok_o0 > $ OR (k.__rok_o0 = $ AND k.__rok_o1 > $) …)
    /// ORDER BY k.__rok_o0, … LIMIT n + 1
    /// ```
    pub(crate) fn wrapped_cursor_sql(
        &self,
        after: Option<&Cursor>,
        limit: u64,
    ) -> Result<(Sql, usize)> {
        const ALIASES: [&str; 16] = [
            "__rok_o0",
            "__rok_o1",
            "__rok_o2",
            "__rok_o3",
            "__rok_o4",
            "__rok_o5",
            "__rok_o6",
            "__rok_o7",
            "__rok_o8",
            "__rok_o9",
            "__rok_o10",
            "__rok_o11",
            "__rok_o12",
            "__rok_o13",
            "__rok_o14",
            "__rok_o15",
        ];
        let mut keyed = self.clone();
        keyed.order = self.keyset_orders();
        // Backward, the outer query walks the keys the other way; the
        // inner `DISTINCT ON` keeps the query's own order, so a joined row
        // keeps the same representative either way.
        let backward = after.is_some_and(Cursor::is_backward);
        let outer: Vec<Order<M>> = keyed
            .order
            .iter()
            .cloned()
            .map(|mut o| {
                if backward {
                    o.direction = flip(o.direction);
                }
                o
            })
            .collect();
        let n = keyed.order.len();
        if n > ALIASES.len() {
            return Err(Error::InvalidQuery(
                "too many sort keys for keyset pagination".into(),
            ));
        }
        let mut sql = keyed.new_sql();
        sql.qualify = true;
        sql.push("SELECT * FROM (SELECT ");
        let multiplies = keyed.multiplies();
        if multiplies {
            sql.push("DISTINCT ON (");
            Self::write_primary_keys(&mut sql);
            sql.push(") ");
        }
        push_columns::<M>(&mut sql);
        keyed.write_order_aliases(&mut sql);
        keyed.write_from_where(&mut sql);
        if multiplies {
            sql.push(" ORDER BY ");
            Self::write_primary_keys(&mut sql);
            for o in &keyed.order {
                sql.push(", ");
                o.write(&mut sql);
            }
        }
        sql.push(r#") "__rok_k""#);
        let key = |i: usize| crate::expr::Term::Column {
            table: "__rok_k",
            name: ALIASES[i],
        };
        if let Some(cursor) = after {
            if cursor.values.len() != n {
                return Err(Error::InvalidCursor(format!(
                    "expected {n} values, got {}",
                    cursor.values.len()
                )));
            }
            let cmp = |i: usize, op: &'static str| Cond::Cmp {
                term: key(i),
                op,
                value: cursor.values[i].clone(),
            };
            let branches = (0..n)
                .map(|i| {
                    let step = match outer[i].direction {
                        crate::expr::Direction::Asc => cmp(i, ">"),
                        crate::expr::Direction::Desc => cmp(i, "<"),
                    };
                    Cond::And((0..i).map(|j| cmp(j, "=")).chain([step]).collect())
                })
                .collect();
            sql.push(" WHERE ");
            Cond::Or(branches).write(&mut sql);
        }
        sql.push(" ORDER BY ").push_list(0..n, ", ", |sql, i| {
            key(i).write(sql);
            outer[i].write_direction(sql);
        });
        sql.push(&format!(" LIMIT {}", limit + 1));
        Ok((sql, n))
    }

    async fn wrapped_cursor_paginate<'e, E: Executor<'e>>(
        self,
        executor: E,
        after: Option<&Cursor>,
        limit: u64,
    ) -> Result<CursorPage<M>> {
        let (sql, n) = self.wrapped_cursor_sql(after, limit)?;
        let rows = exec::fetch_rows(executor, &sql, &[], self.use_replica()).await?;
        let more = rows.len() as u64 > limit;
        // The page's rows in the query's order: a backward read comes in
        // reverse.
        let mut kept: Vec<&_> = rows[..rows.len().min(limit as usize)].iter().collect();
        if after.is_some_and(Cursor::is_backward) {
            kept.reverse();
        }
        let items = kept
            .iter()
            .map(|row| M::from_row(row))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let values = |row: &sqlx::postgres::PgRow| {
            (0..n)
                .map(|i| Value::from_row_column(row, &format!("__rok_o{i}")))
                .collect::<Result<Vec<_>>>()
        };
        let edges = Edges::of(after, more);
        let next = match kept.last() {
            Some(last) if edges.next => Some(Cursor::new(values(last)?)),
            _ => None,
        };
        let prev = match kept.first() {
            Some(first) if edges.prev => Some(Cursor::before(values(first)?)),
            _ => None,
        };
        Ok(CursorPage { items, next, prev })
    }

    pub(crate) fn cursor_select(self, after: Option<&Cursor>, limit: u64) -> Result<Self> {
        let mut order = self.keyset_order();
        // Backward, the query walks the keys the other way; the comparisons
        // below follow the flipped directions, and the caller reverses the
        // rows back into the query's order.
        if after.is_some_and(Cursor::is_backward) {
            for (_, direction) in &mut order {
                *direction = flip(*direction);
            }
        }
        let mut select = self;
        select.order = order
            .iter()
            .map(|(c, d)| match d {
                Direction::Asc => Column::new(c).asc(),
                Direction::Desc => Column::new(c).desc(),
            })
            .collect();
        select.offset = None;
        select.limit = Some(limit + 1);
        if let Some(cursor) = after {
            if cursor.values.len() != order.len() {
                return Err(Error::InvalidCursor(format!(
                    "expected {} values, got {}",
                    order.len(),
                    cursor.values.len()
                )));
            }
            // (a > x) OR (a = x AND b > y) OR … — works for mixed directions.
            let branches = (0..order.len()).map(|i| {
                let eqs = (0..i).map(|j| Column::<M>::new(order[j].0).eq(cursor.values[j].clone()));
                let (column, direction) = order[i];
                let value = cursor.values[i].clone();
                let step = match direction {
                    Direction::Asc => Column::<M>::new(column).gt(value),
                    Direction::Desc => Column::<M>::new(column).lt(value),
                };
                Expr::all_of(eqs.chain([step]))
            });
            select = select.filter(Expr::any_of(branches));
        }
        Ok(select)
    }
}

/// The other direction.
fn flip(direction: Direction) -> Direction {
    match direction {
        Direction::Asc => Direction::Desc,
        Direction::Desc => Direction::Asc,
    }
}

/// Which cursors a page gets, from how it was reached. Forward, a next
/// page exists when a row beyond this page came back, and a previous one
/// when the page was reached through a cursor. Backward, mirrored: the
/// page was reached from a later one, and an earlier page exists when a
/// row beyond it came back.
struct Edges {
    next: bool,
    prev: bool,
}

impl Edges {
    fn of(cursor: Option<&Cursor>, more: bool) -> Self {
        match cursor {
            None => Edges {
                next: more,
                prev: false,
            },
            Some(cursor) if cursor.is_backward() => Edges {
                next: true,
                prev: more,
            },
            Some(_) => Edges {
                next: more,
                prev: true,
            },
        }
    }
}
