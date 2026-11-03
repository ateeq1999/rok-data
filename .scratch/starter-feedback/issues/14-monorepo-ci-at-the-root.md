# 14: The Monorepo's CI runs from the project root

Spec: `docs/specs/starter-feedback.md`

**What to build:** the Monorepo template's API workflow sits at `apps/api/.github/workflows/ci.yml`, and GitHub only runs workflows from the repository root's `.github/workflows`, so a scaffolded Monorepo project's CI never runs. Move the Monorepo's workflows to the template root, each App's jobs with its own `working-directory`, and keep the `ROK_VERSION` check (ticket 01's test) covering them.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] A scaffolded Monorepo project has its workflows under `.github/workflows/` at the root
- [ ] The API job runs the same checks from `apps/api`
- [ ] The template READMEs say where CI lives
