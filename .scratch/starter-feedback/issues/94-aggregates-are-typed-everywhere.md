# 94: `sum`, `avg`, `min` and `max` are typed everywhere

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** the query analyzer types these aggregates by PostgreSQL's rules, in a SELECT, a CTE and a subquery alike:

| Aggregate | Argument | Rust type |
|---|---|---|
| `sum` | `int2`, `int4` | `i64` |
| `sum` | `int8`, `numeric` | `Decimal` |
| `sum` | `float4` / `float8` | `f32` / `f64` |
| `avg` | an integer, `numeric` | `Decimal` |
| `avg` | a float | `f64` |
| `min`, `max` | any | the argument's type |

Each is `Option` (no rows, or only nulls, gives null) unless wrapped in `coalesce(…, non-null)`. A cast still wins. Only what stays untypeable fails, with today's message.

Today `sum(stock)` over an integer column fails in a plain SELECT ("cannot infer the type of result column") and in a CTE ("cannot infer the type of column `n` of CTE `r`"), and so does `min(stock)`. Karibu Pass's `HotelWeekends` needed `(expr)::bigint` inside its CTE (ticket 28).

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Each row of the table infers in a SELECT and in a CTE (analyzer tests)
- [ ] `coalesce(sum(stock), 0)` is non-null with no cast
- [ ] A cast on an aggregate still decides its type
