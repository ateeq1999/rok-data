---
title: Benchmarks
description: The builder-effort bench (criterion) — what is measured, the recorded medians, and the honest scope of these numbers.
updated: 2026-10-29
kind: reference
source: crates/rok-data-core/benches/
---

# Benchmarks

## Agent brief

- **What this measures:** in-memory builder effort only — constructing
  `Expr`/`Select` values and rendering SQL text. No database, no network,
  no I/O. Never quote these as end-to-end performance.
- **Rerun:** `cargo bench -p rok-data-core --bench builder` (criterion);
  compare medians over time, `3s` warmup / `100` samples.

What the workspace has measured so far, honestly scoped: these are
**builder budgets** — the cost of constructing `Expr`/`Select` values and
rendering their SQL text in memory. No database, no network, no I/O.
They exist to keep the builder's per-query overhead visible as the
runtime evolves, not to claim end-to-end performance. Every number below
is one benchmark run on one machine; treat them as orders of magnitude,
tracked over time by rerunning `cargo bench -p rok-data-core` with the
flag when comparing.

## The bench

`cargo bench -p rok-data-core --bench builder` (criterion). The model is
a hand-implemented `Model` (the benches never decode rows).

| Benchmark                     | What it does                                              | Median time        | Throughput        |
| ----------------------------- | --------------------------------------------------------- | ------------------ | ----------------- |
| `single_eq_expr`              | one `Column::eq(7)`                                        | **5.7–6.7 ns**¹    | ~150 Melem/s      |
| `ten_comparisons_chained`     | `.and` chain of 10 comparisons (`eq/ne/like/ilike/in/…`)   | **1.26 µs**        | 791 Kelem/s       |
| `boolean_tree_depth_10`       | alternating and/or tree, 10 levels                        | **1.30 µs**        | 766 Kelem/s       |
| `boolean_tree_depth_50`       | alternating and/or tree, 50 levels (100 leaves)           | **7.40 µs**        | 135 Kelem/s       |
| `full_query_depth_10_render`  | 10 filters + `is_in` of 50 + order + limit, then SQL render | **4.99 µs**       | 200 Kelem/s       |

¹ The one-element bench sits at criterion's resolution limit; across runs
its median moved 5.7 → 6.7 ns (−18/+18 %) while group benches stayed
within ±3 % — treat single-element numbers as noisy by orders of 1 ns.

Reading for a typical workload: a page-size query's filter (`WHERE`
with a handful of comparisons, one `IN`, one order, a limit) costs a
few microseconds of CPU to build. It is negligible against any database
round-trip and far below the cost of *parsing* the equivalent SQL string
on the server side; that is the design margin this benchmark guards.

## Environment of the recorded run

- Windows 11, Intel Core (13th Gen family 6 model 183) — one physical
  machine, codegen defaults (`cargo bench` uses the `bench` profile:
  opt-level 3, no debug symbols for tokio stacks).
- criterion defaults: 3 s warmup, 100 samples, outliers reported rather
  than removed.
- The full numbers from the run (criterion confidence intervals) are in
  the bench output; the table records the medians.

## Known gaps (next candidates)

- `Value` conversion costs (through `Into<Value>`) are inside the above
  counts; a dedicated conversion bench would separate "allocation" from
  "render".
- The engine's codegen (parser pass over a 100+ table schema) is a
  better-complexity benchmark than a micro bench; candidates: the
  `modules/` corpus parse+generate pass in `rok-data-sqlc` and the
  whole-crate generation of `examples/sqlgen-db`, which CI can already
  time.
- Migration loading benchmarks for large `migrations/` trees.
