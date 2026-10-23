//! The model collection: the in-memory shape of a set of models.
//!
//! [`ModelCollection`] wraps a `Vec<M>` under `Deref` and adds the
//! operations a set of rows carries: looking a row up by its key (`find`,
//! `find_or_fail`, `contains`), the set operations on keys (`unique`,
//! `intersect`, `diff`, `only`, `except`, `partition`), and the database
//! round-trips — `load` for the relations beside the rows, `fresh` to
//! re-read the rows themselves, and `to_query` to continue the chain from
//! the rows' keys.

use std::ops::{Deref, DerefMut};
use std::vec::IntoIter;

use crate::key::{IntoKey, key_expr, key_string};
use crate::relation::{IntoInclude, Loaded};
use crate::{Error, Executor, Expr, Journey, Model, Result, Select, Value};

/// A set of models in memory, read by [`Select::get`](crate::Select::get)
/// (or built from any `Vec<M>` with `From` / `collect`). The collection
/// verbs hang off it, and through `Deref` the familiar `Vec` methods work
/// directly:
///
/// ```ignore
/// let admins = User::filter(User::ROLE.eq("admin")).get(&db).await?;
/// assert!(admins.contains_key(7));
/// for admin in admins.iter() { … }        // the Vec shape
/// let now = admins.fresh(&db).await?;     // re-read through the keys
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCollection<M> {
    rows: Vec<M>,
}

impl<M> ModelCollection<M> {
    /// A collection over `rows`, in order.
    pub fn new(rows: Vec<M>) -> Self {
        Self { rows }
    }

    /// The rows, dropping the collection.
    pub fn into_rows(self) -> Vec<M> {
        self.rows
    }
}

impl<M> Deref for ModelCollection<M> {
    type Target = Vec<M>;

    fn deref(&self) -> &Vec<M> {
        &self.rows
    }
}

impl<M> DerefMut for ModelCollection<M> {
    fn deref_mut(&mut self) -> &mut Vec<M> {
        &mut self.rows
    }
}

impl<M> From<Vec<M>> for ModelCollection<M> {
    fn from(rows: Vec<M>) -> Self {
        Self { rows }
    }
}

impl<M, const N: usize> From<[M; N]> for ModelCollection<M> {
    fn from(rows: [M; N]) -> Self {
        Self {
            rows: Vec::from(rows),
        }
    }
}

impl<M> FromIterator<M> for ModelCollection<M> {
    fn from_iter<I: IntoIterator<Item = M>>(rows: I) -> Self {
        Self {
            rows: rows.into_iter().collect(),
        }
    }
}

impl<M> IntoIterator for ModelCollection<M> {
    type Item = M;
    type IntoIter = IntoIter<M>;

    fn into_iter(self) -> IntoIter<M> {
        self.rows.into_iter()
    }
}

impl<M: Model> ModelCollection<M> {
    /// The row whose primary key is `key`, in collection order.
    ///
    /// The key takes the same shapes [`Model::find`] takes: a column value
    /// (`42`, `"a@b.c"`) for a single-column key, a tuple in field order
    /// for a composite key. Integer keys compare across widths, so
    /// `find(42)` finds a `SMALLINT` key 42 as the SQL paths already do.
    pub fn find<K: IntoKey>(&self, key: K) -> Option<&M> {
        let wanted = key.into_key();
        self.rows
            .iter()
            .find(|row| keys_match(&row.key_values(), &wanted))
    }

    /// The row whose primary key is `key`, or
    /// [`Error::NotFound`](crate::Error::NotFound) — the same error
    /// [`Model::find_or_fail`] builds.
    pub fn find_or_fail<K: IntoKey>(&self, key: K) -> Result<&M> {
        let wanted = key.into_key();
        self.rows
            .iter()
            .find(|row| keys_match(&row.key_values(), &wanted))
            .ok_or_else(|| Error::not_found::<M>(Some(key_string(&wanted))))
    }

    /// The primary key of every row, in collection order: one `Vec` per
    /// row holding each key column's value in key order — a single value
    /// for a single-column key, every part of a composite one. To query by
    /// these keys, [`to_query`](Self::to_query) builds the `WHERE` for
    /// either shape.
    pub fn model_keys(&self) -> Vec<Vec<Value>> {
        keys_of(&self.rows)
    }

    /// `true` if some row's primary key is `key` (the key shapes
    /// [`find`](Self::find) takes). Named apart from the slice's
    /// `contains(&row)`, which `Deref` still offers.
    pub fn contains_key<K: IntoKey>(&self, key: K) -> bool {
        self.find(key).is_some()
    }

    /// The rows with distinct primary keys, keeping the first of each:
    /// the first row carrying a key stays, later ones carrying that key
    /// drop out.
    pub fn unique(self) -> Self {
        let mut seen: Vec<Vec<Value>> = Vec::with_capacity(self.rows.len());
        let rows = self
            .rows
            .into_iter()
            .filter(|row| {
                let keys = row.key_values();
                if seen.iter().any(|past| keys_match(past, &keys)) {
                    false
                } else {
                    seen.push(keys);
                    true
                }
            })
            .collect();
        Self::new(rows)
    }

    /// Splits the rows by `predicate` — the matching ones, then the rest —
    /// both keeping collection order.
    pub fn partition<P>(self, mut predicate: P) -> (Self, Self)
    where
        P: FnMut(&M) -> bool,
    {
        let mut matching = Vec::new();
        let mut rest = Vec::new();
        for row in self.rows {
            if predicate(&row) {
                matching.push(row);
            } else {
                rest.push(row);
            }
        }
        (Self::new(matching), Self::new(rest))
    }

    /// The rows whose primary key is in `other`'s keys, keeping
    /// collection order.
    pub fn intersect(self, other: &Self) -> Self {
        let other_keys = keys_of(&other.rows);
        let rows = self
            .rows
            .into_iter()
            .filter(|row| listed(&other_keys, &row.key_values()))
            .collect();
        Self::new(rows)
    }

    /// The rows whose primary key is not in `other`'s keys, keeping
    /// collection order.
    pub fn diff(self, other: &Self) -> Self {
        let other_keys = keys_of(&other.rows);
        let rows = self
            .rows
            .into_iter()
            .filter(|row| !listed(&other_keys, &row.key_values()))
            .collect();
        Self::new(rows)
    }

    /// The rows whose primary key is one of `keys`.
    pub fn only<K: IntoKey>(self, keys: impl IntoIterator<Item = K>) -> Self {
        let wanted: Vec<Vec<Value>> = keys.into_iter().map(IntoKey::into_key).collect();
        let rows = self
            .rows
            .into_iter()
            .filter(|row| listed(&wanted, &row.key_values()))
            .collect();
        Self::new(rows)
    }

    /// The rows whose primary key is none of `keys`.
    pub fn except<K: IntoKey>(self, keys: impl IntoIterator<Item = K>) -> Self {
        let wanted: Vec<Vec<Value>> = keys.into_iter().map(IntoKey::into_key).collect();
        let rows = self
            .rows
            .into_iter()
            .filter(|row| !listed(&wanted, &row.key_values()))
            .collect();
        Self::new(rows)
    }

    /// The query that produces these rows again — the rows' keys
    /// `IN`-matched; a composite key gets one `pk1 = … AND pk2 = …`
    /// condition per row, `OR`ed. The model's own scope applies, so a
    /// soft-deleted row drops out exactly as any other query drops it. A
    /// model without a primary key has nothing to match rows by, so its
    /// query matches nothing.
    pub fn to_query(&self) -> Select<M> {
        let keys = keys_of(&self.rows);
        match M::PRIMARY_KEYS.len() {
            0 => M::filter(Expr::none()),
            1 => M::filter(M::primary_key_column().is_in(keys.into_iter().flatten())),
            _ => M::filter(Expr::any_of(keys.into_iter().map(|values| {
                // `key_values` reads one value per key column, the arity
                // `key_expr` checks, so every row's condition builds.
                key_expr::<M>(values).expect("a row's key carries every key column")
            }))),
        }
    }

    /// The rows re-read through their keys: everything the database knows
    /// they are now, detached from the values this collection carries.
    /// The read applies the model's own scope, and the rows come back in
    /// database order (`to_query().order_by(…)` when the order matters).
    pub async fn fresh<'e, E>(self, executor: E) -> Result<Self>
    where
        E: Executor<'e>,
    {
        let query = self.to_query();
        Ok(query.all(executor).await?.into())
    }

    /// Loads `includes` beside these rows — the batched follow-up queries
    /// a `with`-query runs, over rows already fetched. Returns them as
    /// the loaded view, where each relation is read through the very
    /// constant that asked for it.
    ///
    /// The session takes anything a query takes (the pool, the database,
    /// a transaction). Every include's query runs on that one session,
    /// in the order the includes came.
    ///
    /// ```ignore
    /// let posts = Post::filter(Post::VIEWS.gt(100)).all(&db).await?;
    /// let posts = ModelCollection::from(posts);
    /// let posts = posts.load(&db, [Post::AUTHOR]).await?;
    /// ```
    pub async fn load<'c, S, I, C>(self, session: S, includes: C) -> Result<Loaded<M>>
    where
        S: Into<Journey<'c>>,
        C: IntoIterator<Item = I>,
        I: IntoInclude<M>,
    {
        let journey = session.into();
        let includes: Vec<_> = includes.into_iter().map(IntoInclude::into_packed).collect();
        let mut view = Loaded::new(self.rows);
        crate::query::with::load_includes(journey, &mut view, includes, true).await?;
        Ok(view)
    }

    /// Loads the `includes` not loaded yet — Eloquent's `loadMissing`. A
    /// collection of bare rows has none loaded, so this is
    /// [`load`](Self::load); on the loaded view,
    /// [`Loaded::load_missing`] skips the ones already there, so the
    /// call reads the same at either stage.
    pub async fn load_missing<'c, S, I, C>(self, session: S, includes: C) -> Result<Loaded<M>>
    where
        S: Into<Journey<'c>>,
        C: IntoIterator<Item = I>,
        I: IntoInclude<M>,
    {
        self.load(session, includes).await
    }

    /// The rows serialized as a JSON array — the same shape `Serialize`
    /// gives the collection (when `M` serializes too).
    #[cfg(any(feature = "json", feature = "axum"))]
    pub fn to_json(&self) -> Result<String>
    where
        M: serde::Serialize,
    {
        serde_json::to_string(&self.rows).map_err(Error::from)
    }
}

fn keys_of<M: Model>(rows: &[M]) -> Vec<Vec<Value>> {
    rows.iter().map(|row| row.key_values()).collect()
}

/// `true` when `item`'s key matches one of `listed`.
fn listed(listed: &[Vec<Value>], item: &[Value]) -> bool {
    listed.iter().any(|wanted| keys_match(wanted, item))
}

/// `true` when the two key vectors agree: same length (the primary key
/// column order is fixed) with every value equal. Integer values compare
/// by width (`I64` finds an `I32` key carrying the same 42 — the SQL
/// paths already compare that way), every other variant exactly.
fn keys_match(left: &[Value], right: &[Value]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| same_value(left, right))
}

fn same_value(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (
            Value::I16(_) | Value::I32(_) | Value::I64(_),
            Value::I16(_) | Value::I32(_) | Value::I64(_),
        ) => as_i64(left) == as_i64(right),
        _ => left == right,
    }
}

fn as_i64(value: &Value) -> Option<i64> {
    match value {
        Value::I16(v) => v.map(i64::from),
        Value::I32(v) => v.map(i64::from),
        Value::I64(v) => *v,
        _ => None,
    }
}

/// A collection serializes as the JSON array of its rows — what
/// [`to_json`](ModelCollection::to_json) writes, and what a handler
/// returning the collection sends.
#[cfg(any(feature = "serde", feature = "json"))]
impl<M: serde::Serialize> serde::Serialize for ModelCollection<M> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        self.rows.serialize(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A minimal hand-written model for the pure collection methods; the
    // database rows come from the facade suites.
    #[derive(Debug, Clone, PartialEq)]
    struct User {
        id: i64,
        name: String,
    }

    impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for User {
        fn from_row(_row: &'r sqlx::postgres::PgRow) -> sqlx::Result<Self> {
            unreachable!("the test model is never built from rows")
        }
    }

    impl crate::Hooks for User {}

    impl Model for User {
        const TABLE: &'static str = "users";
        const PRIMARY_KEY: &'static str = "id";
        const COLUMNS: &'static [&'static str] = &["id", "name"];

        fn primary_key(&self) -> Value {
            Value::from(self.id)
        }

        fn values(&self) -> Vec<(&'static str, Value)> {
            vec![
                ("id", Value::from(self.id)),
                ("name", Value::from(self.name.clone())),
            ]
        }
    }

    fn user(id: i64) -> User {
        User {
            id,
            name: format!("u{id}"),
        }
    }

    fn collection() -> ModelCollection<User> {
        ModelCollection::new(vec![user(3), user(1), user(2)])
    }

    fn ids(users: &[User]) -> Vec<i64> {
        users.iter().map(|user| user.id).collect()
    }

    #[test]
    fn find_and_contains_match_keys() {
        let users = collection();
        assert_eq!(users.find(1).map(|row| row.id), Some(1));
        assert_eq!(users.find(99).map(|row| row.id), None);
        assert!(users.contains_key(2));
        assert!(!users.contains_key(99));
        // The missing key's error names the value.
        let missing = match users.find_or_fail(99) {
            Err(error) => error.to_string(),
            Ok(_) => String::new(),
        };
        assert!(missing.contains("99"), "{missing}");
        assert_eq!(users.find_or_fail(3).expect("present").id, 3);
    }

    #[test]
    fn integer_keys_compare_across_widths() {
        // The SQL paths compare integer widths numerically; the in-memory
        // lookup keeps step.
        let users = ModelCollection::from(vec![user(7)]);
        assert!(users.contains_key(7_i64));
        assert_eq!(users.find(7_i64).map(|row| row.id), Some(7));
    }

    #[test]
    fn only_and_except_select_by_keys() {
        let users = collection();
        assert_eq!(ids(&users.only(vec![1, 2])), vec![1, 2]);
        let users = collection();
        assert_eq!(ids(&users.except(vec![1, 2])), vec![3]);
        let users = collection();
        assert_eq!(ids(&users.only(Vec::<i64>::new())), Vec::<i64>::new());
    }

    #[test]
    fn intersect_and_diff_work_on_keys() {
        let users = collection();
        let wanted = ModelCollection::new(vec![user(2), user(99)]);
        assert_eq!(ids(&users.intersect(&wanted)), vec![2]);
        let users = collection();
        assert_eq!(ids(&users.diff(&wanted)), vec![3, 1]);
    }

    #[test]
    fn unique_keeps_the_first_of_each_key() {
        let users: Vec<User> = vec![user(1), user(2), user(1), user(1)];
        assert_eq!(ids(&ModelCollection::from(users).unique()), vec![1, 2]);
        assert_eq!(ids(&collection().unique()), vec![3, 1, 2]);
    }

    #[test]
    fn partition_splits_by_predicate() {
        let users = collection();
        let (small, large) = users.partition(|user| user.id < 3);
        assert_eq!(ids(&small), vec![1, 2]);
        assert_eq!(ids(&large), vec![3]);
    }

    #[test]
    fn model_keys_read_each_rows_key() {
        assert_eq!(
            collection().model_keys(),
            vec![
                vec![Value::I64(Some(3))],
                vec![Value::I64(Some(1))],
                vec![Value::I64(Some(2))]
            ]
        );
    }

    #[test]
    fn to_query_filters_by_keys_in_order() {
        assert_eq!(
            collection().to_query().to_sql().as_str(),
            r#"SELECT "id", "name" FROM "users" WHERE "id" IN ($1, $2, $3)"#
        );
        // An empty collection matches nothing.
        let empty: ModelCollection<User> = ModelCollection::new(Vec::new());
        assert_eq!(
            empty.to_query().to_sql().as_str(),
            r#"SELECT "id", "name" FROM "users" WHERE FALSE"#
        );
    }

    #[test]
    fn the_vec_shapes_work() {
        // FromIterator, Deref, IntoIterator and into_rows all speak Vec.
        let users: ModelCollection<User> = vec![user(1), user(2)].into_iter().collect();
        assert_eq!(users.len(), 2);
        assert_eq!(users.first().map(|user| user.id), Some(1));
        assert_eq!(
            users.into_iter().map(|user| user.id).collect::<Vec<i64>>(),
            vec![1, 2]
        );
        assert_eq!(collection().into_rows().len(), 3);
    }
}
