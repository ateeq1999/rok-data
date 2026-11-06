# 73: Integer widths in the Clients

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** the contract reader keeps an integer's `format` (it reads past it today, so every integer is `i64` in the Rust Client). `format: int32` is Rust `i32`; `int64` or no format stays `i64`. utoipa writes `int32` for both `i16` and `i32` and has no `int16`, so a `smallint` arrives as `i32`. A `minimum`/`maximum` is carried where the Client has a place for it. Dart's `int` and TypeScript's `number` are unchanged (ticket 18's zod schemas add the bounds on the TypeScript side). Engine ADR 0027 is amended.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] An `int32` property is `i32` in the Rust Client, an `int64` one `i64` (emission tests)
- [ ] api-app's document emits a Rust Client that builds
