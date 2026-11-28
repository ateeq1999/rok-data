# 19: The web App builds its forms on the Client's schemas

Spec: `docs/specs/starter-feedback.md` (Typed web data)

**What to build:** in the Monorepo web App, for every feature, not only auth:
- **Deleted duplicates:** `features/*/schemas.ts` drops the response schemas the Client now emits (`tokenResponseSchema`, `totpRequiredSchema`, `loginResponseSchema` and the like) and imports them from the Client.
- **Form schemas** (glossary: rok-templates' *Form schema*):
  - Each is built on the Client's request schema with `.extend`/`.pick`/`.refine`. It adds only what the screen needs: confirm fields, its own wording, and the TOTP and recovery-code formats.
  - It doesn't restate a rule the request schema carries (ticket 97). Login's "no length hint" comes from `LoginDto` itself (ticket 98).
- **Adapters, not URLs:**
  - Each feature's `api.ts` (`authApi` and the rest) stays the feature's hand-written adapter, but calls the generated Client.
  - It holds no URL strings and no `.parse`, since the Client parses responses itself.
  - Web-only options such as `skipExpire` stay in the adapter.
- **zod pin:** `zod` is pinned exactly, and the web gate fails if it disagrees with the zod major in the Client's header.
- **Error:** callers that show errors handle `ContractError`.

**Blocked by:** a rok release carrying 18 and 97; ticket 98

**Status:** blocked

- [ ] No response schema is written twice, and no form schema restates a rule its request schema carries
- [ ] No feature's `api.ts` holds a URL string
- [ ] The auth form tests still pass; the web gate passes
- [ ] `zod` exact pin checked by the gate
