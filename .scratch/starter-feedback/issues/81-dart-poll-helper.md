# 81: A polling helper in the Dart Client

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** the Dart Client runtime gains `poll(() => call(), every: Duration, until: (T value) => bool)`, which returns a `Stream<T>`. It calls once at once, then every `every`, stops when `until` is true, pauses while the App is in the background, and calls once on resume. The TypeScript side documents TanStack Query's `refetchInterval` for the same job. No per-operation codegen. Topics (ticket 74) replace it where a push channel fits.

Karibu Pass's `BillCubit` and `CheckoutCubit` each own a `Timer.periodic` with its own stop rule and lifecycle hook (ticket 23).

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] `poll` stops when `until` holds, and pauses and resumes with the App lifecycle (Dart tests)
- [ ] Documented in the Dart Client's README and the TypeScript guide
