# 19: The web App builds its forms on the Client's schemas

Spec: `docs/specs/starter-feedback.md` (Typed web data)

**What to build:** in the Monorepo web App:
- **Deleted duplicates:** `features/*/schemas.ts` drops the response schemas the Client now emits (`tokenResponseSchema`, `totpRequiredSchema`, `loginResponseSchema` and the like) and imports them from the Client.
- **Form schemas:** built on the generated request schemas with `.extend`/`.pick`/`.refine`, adding messages and confirm fields. Login's deliberate "no length hint" and the TOTP and recovery-code formats stay hand-written.
- **zod pin:** `zod` is pinned exactly, and the web gate fails if it disagrees with the zod major in the Client's header.
- **Error:** callers that show errors handle `ContractError`.

**Blocked by:** a rok release carrying 18 (merged after v0.9.0); grpc-api-template/08 is done

**Status:** blocked

- [ ] No response schema is written twice
- [ ] The auth form tests still pass; the web gate passes
- [ ] `zod` exact pin checked by the gate
