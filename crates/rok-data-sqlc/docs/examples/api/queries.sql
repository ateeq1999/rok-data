-- SQL the data layer doesn't generate, called by ProjectsService.

-- name: OpenTask :execrows
UPDATE projects SET open_tasks = open_tasks + 1
WHERE id = $1 AND owner_id = $2;

-- name: ProjectStats :one
SELECT count(*) AS projects, coalesce(sum(open_tasks), 0)::BIGINT AS open_tasks
FROM projects
WHERE owner_id = $1;

-- name: ProjectNames :many
-- returns: crate::shared::listing::ProjectName
SELECT id, name FROM projects WHERE owner_id = $1 ORDER BY name;

-- One query for both sides of a moment: `$2` switches the filter, and each
-- sort direction is its own `CASE` (a `CASE` can't switch `ASC`/`DESC`).
-- name: ProjectsAround :many
-- param: $2 after
-- param: $3 at
SELECT id, name, created_at FROM projects
WHERE owner_id = $1
  AND CASE WHEN $2::bool THEN created_at >= $3 ELSE created_at < $3 END
ORDER BY CASE WHEN $2 THEN created_at END ASC,
         CASE WHEN NOT $2 THEN created_at END DESC;
