//! Builder benchmarks: how long the SQL builder takes to construct
//! `Expr`/`Select` values and render their SQL text. No database, no I/O —
//! everything is building in-memory query values or composing SQL strings.
//!
//! Run it with `cargo bench -p rok-data-core --bench builder`.
//!
//! The workspace's `missing_docs` lint is off here: this file documents every
//! item it owns, and `criterion_group!` expands to an undocumented public
//! function that a file-level attribute cannot reach.

#![allow(missing_docs)]

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};

use rok_data_core::{Column, Expr, Hooks, Model, Select, Value};

/// A hand-implemented model: the benches need `Expr<M>`/`Select<M>` typing
/// only, never row decoding.
struct Note {
    id: i64,
    text: String,
}

impl Model for Note {
    const TABLE: &'static str = "notes";
    const PRIMARY_KEY: &'static str = "id";
    type Key = i64;
    const COLUMNS: &'static [&'static str] = &["id", "text"];

    fn primary_key(&self) -> Value {
        self.id.into()
    }

    fn values(&self) -> Vec<(&'static str, Value)> {
        vec![("id", self.id.into()), ("text", self.text.clone().into())]
    }
}

impl Hooks for Note {}

impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for Note {
    fn from_row(_row: &'r sqlx::postgres::PgRow) -> std::result::Result<Self, sqlx::Error> {
        unimplemented!("the benches never decode rows; Select<M> requires the FromRow bound")
    }
}

/// Every single-column comparison the builder offers, chained.
fn comparisons() -> Expr<Note> {
    let id = Column::<Note>::new("id");
    let text = Column::<Note>::new("text");
    id.eq(7)
        .and(text.ne("draft"))
        .and(text.contains("urgent"))
        .and(text.not_like("%zzz%"))
        .and(text.ilike("Urg%"))
        .and(id.gt(0))
        .and(text.starts_with("re:"))
        .and(text.ends_with("!"))
        .and(id.is_not_null())
        .and(text.is_in(["a", "b", "c"]))
}

/// A boolean tree alternating and/or, five levels deep.
fn boolean_tree(depth: u32) -> Expr<Note> {
    let id = Column::<Note>::new("id");
    let mut expr = id.eq(0_i64);
    for level in 0..depth {
        let leaf = move || {
            if level % 2 == 0 {
                id.gte(level as i64)
            } else {
                id.lt((depth + level) as i64)
            }
        };
        expr = expr.and(leaf()).or(leaf());
    }
    expr
}

/// The builder overhead of a full query: filters, order, limit, render.
fn full_query(depth: u32) -> String {
    let mut select = Select::<Note>::new()
        .all_tenants()
        .order_by(Column::new("text").desc());
    for level in 0..depth {
        select = select.filter(Column::new("id").eq(black_box(level as i64)));
    }
    let rendered = select
        .filter(Column::new("text").is_in(0..50_i64))
        .limit(10)
        .to_sql();
    rendered.to_string()
}

fn bench_builder(c: &mut Criterion) {
    let mut group = c.benchmark_group("builder");
    group.throughput(Throughput::Elements(1));

    group.bench_function("single_eq_expr", |b| {
        let column = Column::<Note>::new("id");
        b.iter(|| column.eq(black_box(7_i64)))
    });
    group.bench_function("ten_comparisons_chained", |b| b.iter(comparisons));
    group.bench_function("boolean_tree_depth_10", |b| {
        b.iter(|| boolean_tree(black_box(10)))
    });
    group.bench_function("boolean_tree_depth_50", |b| {
        b.iter(|| boolean_tree(black_box(50)))
    });
    group.bench_function("full_query_depth_10_render", |b| {
        b.iter(|| full_query(black_box(10)))
    });

    group.finish();
}

criterion_group!(benches, bench_builder);
criterion_main!(benches);
