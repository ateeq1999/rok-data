# 06: A Blueprint records the API framework and the Modules left out

Spec: `docs/specs/starter-feedback.md` · workspace ADRs 0029, 0033

**What to build:** a Blueprint holds `-w` and the `--without` list beside the Template, its Integrations and its init options, and a project created from it comes out with the same framework and Modules. If Blueprints are not built yet, this ticket adds the two fields to their spec instead.

**Blocked by:** 02, 04

**Status:** blocked

- [ ] A Blueprint round-trips the framework and the left-out Modules
- [ ] A project created from it matches one created with the same flags
