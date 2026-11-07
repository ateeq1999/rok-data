-- Generated into tests/fixture-app/src/db/queries/shelving.rs: data-modifying
-- CTEs, one statement that changes several tables at once. `gone` reads the
-- stock `live` saw before zeroing it; the final `INSERT … SELECT` reads
-- `gone`'s `RETURNING` columns. If the insert fails, so does the update.

-- name: ShelveCategory :execrows
WITH live AS (
    SELECT id, stock_on_hand FROM products WHERE category_id = $1 AND NOT discontinued
),
gone AS (
    UPDATE products SET discontinued = true, stock_on_hand = 0
    FROM live WHERE products.id = live.id
    RETURNING products.id, live.stock_on_hand AS returned
)
INSERT INTO stock_levels (product_id, warehouse_id, quantity, counted_at)
SELECT gone.id, $2, gone.returned, now()::timestamp FROM gone;

-- The same statement with `RETURNING`: the final statement gives the rows.
-- name: ShelveCategoryReturning :many
WITH live AS (
    SELECT id, stock_on_hand FROM products WHERE category_id = $1 AND NOT discontinued
),
gone AS (
    UPDATE products SET discontinued = true, stock_on_hand = 0
    FROM live WHERE products.id = live.id
    RETURNING products.id, live.stock_on_hand AS returned
)
INSERT INTO stock_levels (product_id, warehouse_id, quantity, counted_at)
SELECT gone.id, $2, gone.returned, now()::timestamp FROM gone
RETURNING product_id, quantity;
