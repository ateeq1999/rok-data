-- The reads every feature may call (`db::queries::shared::accounts`):
-- the notes and the labels services both call `AccountEmail`.

-- name: AccountEmail :maybe_one
SELECT email FROM accounts WHERE id = $1;
