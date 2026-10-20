# Glossary Map

## Contexts

- [Workspace](./GLOSSARY.md): the `rok` CLI surface, the unified pipeline (schema dir → data layer → running API), and its product terms
- [rok-data-sqlc](./crates/rok-data-sqlc/GLOSSARY.md): the resource-module/OpenAPI generator engine's internal vocabulary (Table, Resource, API feature, Template, Drift …)

## Relationships

- **Workspace → rok-data-sqlc**: the workspace pipeline consumes the engine's vocabulary unchanged (Resource, API feature, Template, Convention/Preset, Drift); the two-engine split from ADR 0006 collapses into one data layer, so conflicts resolve in favor of the engine glossary's engine-internal terms and this glossary's pipeline terms.
