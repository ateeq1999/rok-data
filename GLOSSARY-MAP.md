# Glossary Map

## Contexts

- [Workspace](./GLOSSARY.md): the `rok` CLI surface, the unified pipeline (schema dir → data layer → running API), and its product terms
- [rok-data-sqlc](./crates/rok-data-sqlc/GLOSSARY.md): the resource-module/OpenAPI generator engine's internal vocabulary (Table, Resource, API feature, Scaffolded file, Drift …)
- [rok-templates](./crates/rok-templates/GLOSSARY.md): the starter projects `init --template` copies and what they ship (Template, Capability, Shared file)

## Relationships

- **Workspace → rok-data-sqlc**: the workspace pipeline consumes the engine's vocabulary unchanged (Resource, API feature, Convention/Preset, Drift); the two-engine split from ADR 0006 collapses into one data layer, so conflicts resolve in favor of the engine glossary's engine-internal terms and this glossary's pipeline terms.
- **rok-templates → rok-data-sqlc**: a Template is copied by the engine's `init` and holds API features and scaffolded files in the engine's sense; a Capability is used by API features but is never generated (workspace ADR 0023). rok-templates decides which files a Template's project gets; the engine decides how they are written (workspace ADR 0027).
