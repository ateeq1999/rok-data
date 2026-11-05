# 58: Signed webhooks, and a provider that calls back

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:**
- **The extractor:** the templates gain a `SignedWebhook<V>` extractor in the security Capability. It hands a handler the request's headers and raw body once a verifier `V` you implement accepts the signature, and rejects with 401 otherwise.
- **The recipe:** a guide recipe, "a provider that calls back", shows the shape `mail` already uses:
  - a port
  - a real and a fake transport chosen by `<NAME>_TRANSPORT`
  - a fake that signs its callbacks exactly as the provider does
  - a test helper that swaps the fake into `AppState`
- **No vendor** is shipped.

**Blocked by:** grpc-api-template/08 (the gRPC Template shipped)

**Status:** blocked

- [ ] `SignedWebhook` accepts a correctly signed body and refuses a tampered one (tests)
- [ ] The recipe's code compiles (doc test or fixture)
- [ ] The Shared files stay identical across templates
