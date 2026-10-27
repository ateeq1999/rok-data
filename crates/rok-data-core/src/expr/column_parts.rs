//! What a column offers beyond plain comparisons: its date and time parts,
//! JSON array lengths and paths, comparisons with another column, and
//! `NULL`-aware equality.

use crate::{Model, Value};

use super::{Column, Cond, Expr, Projection};

impl<M: Model> Column<M> {
    /// The date of a timestamp column (`CAST(column AS DATE)`), to compare
    /// with a date: `Post::PUBLISHED_AT.date().eq(today)`.
    pub fn date(self) -> Projection<M> {
        self.cast_to("DATE")
    }

    /// The time of day of a timestamp column (`CAST(column AS TIME)`).
    pub fn time(self) -> Projection<M> {
        self.cast_to("TIME")
    }

    /// The year of a date or timestamp column, as an `INTEGER`:
    /// `Post::PUBLISHED_AT.year().eq(2026)`.
    pub fn year(self) -> Projection<M> {
        self.extract("YEAR")
    }

    /// The month (1–12) of a date or timestamp column, as an `INTEGER`.
    pub fn month(self) -> Projection<M> {
        self.extract("MONTH")
    }

    /// The day of the month (1–31) of a date or timestamp column, as an
    /// `INTEGER`.
    pub fn day(self) -> Projection<M> {
        self.extract("DAY")
    }

    /// The hour (0–23) of a timestamp or time column, as an `INTEGER`.
    pub fn hour(self) -> Projection<M> {
        self.extract("HOUR")
    }

    fn cast_to(self, sql_type: &str) -> Projection<M> {
        Projection::raw::<Value>(format!("CAST({} AS {sql_type})", self.qualified()), [])
    }

    fn extract(self, field: &str) -> Projection<M> {
        Projection::raw::<Value>(
            format!(
                "CAST(EXTRACT({field} FROM {}) AS INTEGER)",
                self.qualified()
            ),
            [],
        )
    }

    /// The length of a JSONB array column (`jsonb_array_length(column)`):
    /// `Post::TAGS.json_array_len().gt(3)`.
    pub fn json_array_len(self) -> Projection<M> {
        Projection::raw::<Value>(format!("jsonb_array_length({})", self.qualified()), [])
    }

    /// The length of the JSONB array at a nested `path`
    /// (`jsonb_array_length(column #> $1)`):
    /// `Event::DATA.json_array_len_at(["user", "roles"]).gte(1)`.
    pub fn json_array_len_at<K: Into<String>>(
        self,
        path: impl IntoIterator<Item = K>,
    ) -> Projection<M> {
        let path: Vec<String> = path.into_iter().map(Into::into).collect();
        Projection::raw(
            format!("jsonb_array_length({} #> ?)", self.qualified()),
            [path],
        )
    }

    /// The JSONB column has a value at the nested `path`
    /// (`column #> $1 IS NOT NULL`): `Event::DATA.json_has_path(["user", "id"])`.
    pub fn json_has_path<K: Into<String>>(self, path: impl IntoIterator<Item = K>) -> Expr<M> {
        let path: Vec<String> = path.into_iter().map(Into::into).collect();
        Expr::raw(format!("{} #> ? IS NOT NULL", self.qualified()), [path])
    }

    /// The JSONB value at the nested `path` contains `json`
    /// (`column #> $1 @> $2`):
    /// `Event::DATA.json_contains_at(["user"], json!({"admin": true}))`.
    #[cfg(feature = "json")]
    pub fn json_contains_at<K: Into<String>>(
        self,
        path: impl IntoIterator<Item = K>,
        json: serde_json::Value,
    ) -> Expr<M> {
        let path: Vec<String> = path.into_iter().map(Into::into).collect();
        Expr::raw(
            format!("{} #> ? @> ?", self.qualified()),
            [Value::from(path), Value::from(json)],
        )
    }

    fn columns(self, op: &'static str, other: Column<M>) -> Expr<M> {
        Expr::new(Cond::Columns {
            left: (M::TABLE, self.name()),
            op,
            right: (M::TABLE, other.name()),
        })
    }

    /// `column = other`, two columns of the same row.
    pub fn eq_column(self, other: Column<M>) -> Expr<M> {
        self.columns("=", other)
    }

    /// `column <> other`
    pub fn ne_column(self, other: Column<M>) -> Expr<M> {
        self.columns("<>", other)
    }

    /// `column > other`: `Task::UPDATED_AT.gt_column(Task::CREATED_AT)`.
    pub fn gt_column(self, other: Column<M>) -> Expr<M> {
        self.columns(">", other)
    }

    /// `column >= other`
    pub fn gte_column(self, other: Column<M>) -> Expr<M> {
        self.columns(">=", other)
    }

    /// `column < other`
    pub fn lt_column(self, other: Column<M>) -> Expr<M> {
        self.columns("<", other)
    }

    /// `column <= other`
    pub fn lte_column(self, other: Column<M>) -> Expr<M> {
        self.columns("<=", other)
    }

    /// `lower(column) = lower(value)`: equality ignoring case, the form a
    /// unique index on `lower(column)` compares.
    pub fn eq_ignore_case(self, value: impl Into<Value>) -> Expr<M> {
        Expr::raw(
            format!("lower({}) = lower(?)", self.qualified()),
            [value.into()],
        )
    }

    /// `column IS NOT DISTINCT FROM value`: equality where two `NULL`s are
    /// equal, so `None` matches the rows whose column is `NULL`.
    pub fn is_not_distinct_from(self, value: impl Into<Value>) -> Expr<M> {
        self.cmp("IS NOT DISTINCT FROM", value)
    }

    /// `column IS DISTINCT FROM value`: inequality where a `NULL` differs
    /// from every value and equals another `NULL`.
    pub fn is_distinct_from(self, value: impl Into<Value>) -> Expr<M> {
        self.cmp("IS DISTINCT FROM", value)
    }
}
