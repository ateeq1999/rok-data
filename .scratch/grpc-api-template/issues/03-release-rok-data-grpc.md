# 03: Release rok-data with gRPC generation

Spec: `docs/specs/grpc-api-template.md`

**What to build:** a rok-data release whose generator knows `framework = "grpc"`, so rok-templates' gates (which build the generator at `rok-data.pin`) can generate the `grpc-api` template. The version-bump PR follows the 0.6/0.7 procedure (pins, regenerated examples, CHANGELOG section, migration guide); the tag goes on the merge commit.

**Blocked by:** 02

**Status:** blocked

- [ ] Release PR merged, tag on its merge commit, all crates published
