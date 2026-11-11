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

-- name: TasksWithProject :many
-- A query reads any table: one join gives each task its project's name.
SELECT t.id, t.title, t.done, p.name AS project_name
FROM tasks t
JOIN projects p ON p.id = t.project_id
WHERE t.owner_id = $1
ORDER BY t.created_at;
