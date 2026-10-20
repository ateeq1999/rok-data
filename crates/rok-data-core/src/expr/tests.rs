//! The expression layer's own tests.

use super::*;
use crate::Model;
use crate::sql::Sql;
use crate::value::Value;

struct T;
const A: Column<T> = Column::new("a");
const B: Column<T> = Column::new("b");

impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for T {
    fn from_row(_: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(T)
    }
}
impl crate::Hooks for T {}
impl Model for T {
    const TABLE: &'static str = "t";
    const PRIMARY_KEY: &'static str = "a";
    const COLUMNS: &'static [&'static str] = &["a", "b"];
    fn primary_key(&self) -> Value {
        Value::I32(None)
    }
    fn values(&self) -> Vec<(&'static str, Value)> {
        Vec::new()
    }
}

fn render(e: Expr<T>) -> String {
    let mut sql = Sql::new();
    e.cond.write(&mut sql);
    sql.as_str().to_owned()
}

#[test]
fn renders_conditions() {
    assert_eq!(render(A.eq(1)), r#""a" = $1"#);
    assert_eq!(render(A.eq(1).and(B.gt(2))), r#"("a" = $1 AND "b" > $2)"#);
    assert_eq!(
        render(A.eq(1).or(B.is_null()).and(!A.is_in([1, 2]))),
        r#"(("a" = $1 OR "b" IS NULL) AND NOT ("a" IN ($2, $3)))"#
    );
    assert_eq!(render(A.is_in(Vec::<i32>::new())), "FALSE");
    assert_eq!(render(A.not_in(Vec::<i32>::new())), "TRUE");
    assert_eq!(render(A.between(1, 5)), r#""a" BETWEEN $1 AND $2"#);
    assert_eq!(render(Expr::all()), "TRUE");
    assert_eq!(render(Expr::any_of([])), "FALSE");
    assert_eq!(render(Expr::raw("lower(a) = ?", ["x"])), "(lower(a) = $1)");
    assert_eq!(
        render(A.sum().cast("BIGINT").gt(5)),
        r#"CAST(SUM("a") AS BIGINT) > $1"#
    );
    assert_eq!(render(Projection::count_all().gte(2)), "COUNT(*) >= $1");
    assert_eq!(
        render(B.count_distinct().eq(1)),
        r#"COUNT(DISTINCT "b") = $1"#
    );
}

#[test]
fn qualifies_columns_on_request() {
    let mut sql = Sql::new();
    sql.qualify = true;
    A.eq(1)
        .and(B.is_null())
        .and(A.is_in([2]))
        .cond
        .write(&mut sql);
    assert_eq!(
        sql.as_str(),
        r#"("t"."a" = $1 AND "t"."b" IS NULL AND "t"."a" IN ($2))"#
    );
}

#[test]
fn escapes_like_patterns() {
    let mut sql = Sql::new();
    A.contains("50%_off").cond.write(&mut sql);
    assert_eq!(sql.params()[0], Value::from(r"%50\%\_off%"));
}
