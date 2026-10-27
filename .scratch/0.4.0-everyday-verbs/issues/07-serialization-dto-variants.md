# 07: Serialization — DTO variants (Public/WithExtras), date formats, hidden/visible via DTOs

**What to build (as specified):** `UserDto` / `UserPublicDto` / `UserWithExtrasDto` per model from the generator, a global `serialize_date` format and per-field formats through `cast`.

**Blocked by:** 05.

**Status:** done (2026-10-27) — resolved by existing mechanisms, by decision with the user

- [x] **The public shape without sensitive columns** — already generated: every generated API feature answers a response DTO that leaves out the table's `[api.hidden]` columns (the templates hide `password_hash`, `totp_secret`, …), and its OpenAPI schema says so. A second, convention-named `PublicDto` (guessing sensitivity from column names) would compete with that explicit list
- [x] **Computed fields** — a hand-written DTO with `#[serde(flatten)]` over the generated one, or a service method; no `#[rok(accessor)]` layer
- [x] **Date formats** — serde's field attributes (`#[serde(with = …)]`, `serialize_with`) on the model or DTO; no `serialize_date` attribute
- [x] No runtime hidden/visible/appends (as specified)
