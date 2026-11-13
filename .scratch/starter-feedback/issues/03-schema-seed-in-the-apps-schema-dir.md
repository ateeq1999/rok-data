# 03: `-s` seeds the schema directory `rok gen` reads

Spec: `docs/specs/starter-feedback.md`

**What to build:** with a Monorepo template, `rok init -s schema.sql` writes the file into the schema directory the project's `rok.toml` names for the App `rok gen` generates from (the `api` App's), and the init summary prints the path. Single-app templates keep `schema/0001_init.sql`.

**Blocked by:** grpc-api-template/08 (the gRPC Template shipped)

**Status:** ready-for-agent

- [ ] Monorepo: the seed lands where `rok gen` reads it, and `rok gen` uses it
- [ ] The summary names the path; the `--schema` help says where it goes
