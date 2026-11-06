-- Generated into tests/fixture-app/src/db/queries/bookings.rs: a `:page`
-- query, written once with its `WHERE` and `ORDER BY`, returns
-- `rok_data::Page` with the rows of one page and the total of every page.

-- name: PartnerBookings :page
-- param: $4 search
SELECT id, reference, guest_name, guest_email, offer_title, starts_on
FROM bookings
WHERE partner_id = $1
  AND starts_on BETWEEN $2 AND $3
  AND (reference ILIKE $4 OR guest_name ILIKE $4 OR guest_email ILIKE $4 OR offer_title ILIKE $4)
ORDER BY starts_on DESC, id;
