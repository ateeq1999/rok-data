# 04: Generated TS Client; the web App talks through it

**What to build:** The Clients become real, per ADR 0027 — rok emits them itself from the api's `openapi.json`, not from the engine's model (hand-written auth/2FA routes exist only in the spec). Ticket for the TS one: the api App gains a genuine `openapi.json` artifact (the served document written out; refreshed only through the sanctioned expected-snapshot mechanism), rok reads the OpenAPI 3.1 subset utoipa emits — no more — and emits `clients/ts`: typed calls for every path, plus TanStack Query `queryOptions`/`mutationOptions` factories, keeping the web starter's `ApiError`/`FieldIssue` error shape. The web App's hand-rolled `http.ts` and per-feature api modules are replaced by the generated client: its screens now call through it, and the gate still passes. Init emits `clients/ts` into a fresh scaffold; `rok gen` regenerates it (and re-exports the artifact first, so contract changes flow through); generated-marker rules apply.

**Blocked by:** 03 (web App imported — its data layer is what this ticket replaces).

**Status:** ready-for-agent

- [ ] The api App's served document is exported to a real `openapi.json`; a template-level check keeps it honest with permitted regeneration (never hand-edited), same practice as the repo's other expected snapshots
- [ ] rok parses the utoipa subset (paths, methods, schemas, request bodies, statuses, auth marker) and rejects gracefully what it doesn't support, naming the unsupported construct
- [ ] The TS client: typed calls with request/response types per operation,auth/session concerns preserved (bearer/cookie handling matching the web starter's behavior today), `ApiError` with `FieldIssue` for validation responses, and `queryOptions`/`mutationOptions` factories usable by TanStack Query callers
- [ ] The web App's data layer is generated code: the starter's own HTTP module and feature api files are gone; screens call through the client; a contract change in the api spec changes the client output
- [ ] Init emits `clients/ts` (as generated files, with whatever marker/marker-policy implications regeneration expects); `rok gen` re-exports + re-emits it; drift behavior for the client is defined and tested
- [ ] The web typecheck/tests gate passes with the client in the loop, and a spec-driven regeneration round-trips through the gate; format/clippy/engine/rok-cli/rok-templates suites green
