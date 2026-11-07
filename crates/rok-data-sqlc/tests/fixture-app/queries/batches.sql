-- Generated into tests/fixture-app/src/db/queries/batches.rs: set-returning
-- functions in FROM. One statement writes many rows: one per week between
-- two dates, one per array element, one per JSON object.

-- name: CopyHoldsForward :execrows
-- `generate_series` over dates yields `timestamp with time zone`; the cast
-- makes each week a `date` again.
INSERT INTO weekly_holds (offer_id, week_of, rooms)
SELECT $1, week::date, $2
FROM generate_series($3::date, $4::date, '7 days') AS week;

-- name: AddLinesFromArrays :execrows
INSERT INTO order_lines (order_id, item_id, quantity)
SELECT $1, l.item_id, l.quantity
FROM unnest($2::uuid[], $3::int[]) AS l(item_id, quantity);

-- name: AddLinesFromJson :execrows
-- param: $2 lines
INSERT INTO order_lines (order_id, item_id, quantity)
SELECT $1, l.item_id, l.quantity
FROM jsonb_to_recordset($2) AS l(item_id uuid, quantity int);

-- name: HoldWeeks :many
SELECT week_of, rooms FROM weekly_holds WHERE offer_id = $1 ORDER BY week_of;

-- name: OrderQuantities :many
SELECT q.n, q.idx FROM unnest($1::int[]) WITH ORDINALITY AS q(n, idx);
