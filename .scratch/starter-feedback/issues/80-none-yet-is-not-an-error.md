# 80: "None yet" is an answer, not an error

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** an endpoint whose current value may not exist yet (`GET /me/tables/{id}/bill` before the diner orders) answers 200 with a `null` body. It declares an `Option<BillResponse>` response, which utoipa writes as a nullable schema. A 404 keeps meaning "no such resource".
- **Engine:** a nullable top-level response schema becomes a nullable return type in every Client: Dart `Future<BillResponse?>`, TypeScript `Promise<BillResponse | null>` (TanStack Query's `data` is `null`, not `isError`), and Rust `Result<Option<BillResponse>, ApiError>`. Check what each Client emits today and fix the gaps.
- **Templates:** the guide line "an absent current value is 200 with `null`; a missing resource is 404", with an example handler.

Karibu Pass's repositories catch the 404 and return `null` by hand (ticket 23).

**Blocked by:** None (can start immediately)

**Status:** blocked (engine part done on `sf/80`; the guide part is rok-templates work)

- [x] An `Option<T>` response emits a nullable return in each Client (emission tests): utoipa writes `body = Option<BillResponse>` as `oneOf: [{type: null}, {$ref}]`. Dart (`Future<BillResponse?>`) and Rust (`Result<Option<types::BillResponse>, ApiError>`) already returned it; TypeScript wrote `Promise<null | BillResponse>` and `z.union([z.null(), …])`, now `Promise<BillResponse | null>` and `BillResponseSchema.nullable()`. Each runtime already decodes a `null` body. Tests: `a_nullable_success_answers_*` in `clients/{ts,dart,rust}/tests.rs`; the wire standard guide says 404 is "no such resource" and `null` is "none yet".
- [ ] The guide's example compiles in a template gate: rok-templates work, not built here. It needs no rok release: the released Clients already return a nullable type (TypeScript spells it `null | BillResponse` until the next release). Add the guide line and an example handler declaring `body = Option<BillResponse>` on rok-templates `spec/starter-feedback`.
