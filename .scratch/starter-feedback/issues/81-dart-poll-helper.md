# 81: A polling helper in the Dart Client

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** the Dart Client runtime gains `poll(() => call(), every: Duration, until: (T value) => bool)`, which returns a `Stream<T>`. It calls once at once, then every `every`, stops when `until` is true, pauses while the App is in the background, and calls once on resume. The TypeScript side documents TanStack Query's `refetchInterval` for the same job. No per-operation codegen. Topics (ticket 74) replace it where a push channel fits.

Karibu Pass's `BillCubit` and `CheckoutCubit` each own a `Timer.periodic` with its own stop rule and lifecycle hook (ticket 23).

**Blocked by:** None (can start immediately)

**Status:** done

- [x] `poll` stops when `until` holds, and pauses and resumes with the App lifecycle (Dart tests: run with fake_async against the emitted runtime in a scratch package; the engine tests check the emitted runtime, export and README)
- [x] Documented in the Dart Client's README and the TypeScript Client's README (`refetchInterval`)

The App lifecycle reaches `poll` through `AppLifecycle.foreground`, which the
Flutter App sets from `AppLifecycleListener` (the Client is pure Dart, so it
cannot import Flutter). Switching Karibu Pass's `BillCubit` and
`CheckoutCubit` from `Timer.periodic` to `poll` in rok-templates waits on a
rok release: the templates generate their Client with the released `rok`.
