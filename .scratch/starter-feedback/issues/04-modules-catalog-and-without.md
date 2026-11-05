# 04: Modules in the catalog, and `--without`

Spec: `docs/specs/starter-feedback.md` · workspace ADR 0033

**What to build:** the rok-templates catalog declares a Template's Modules (name, files per App, wiring points, the Modules each needs), and `rok init … --without <MODULE,…>` drops those files and wiring lines. The first Module is `api-keys`, end to end across the API and web Apps. An unknown Module, or one a kept Module needs, is a usage error. The gates build the default and the all-out combination.

**Blocked by:** grpc-api-template/08 (the gRPC Template shipped)

**Status:** blocked

- [ ] `--without api-keys` scaffolds a project that compiles and passes its tests
- [ ] Unknown and needed-by-another Modules are usage errors naming them
- [ ] Gates: default and all-out combinations green
- [ ] Glossary, CLI reference and guide describe Modules
