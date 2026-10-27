# 07: Serialization — DTO variants (Public/WithExtras), date formats, hidden/visible via DTOs

**What to build:**
1. **DTO variants from generator**:
   - `UserDto` — all columns (current `UserDto`)
   - `UserPublicDto` — visible subset (excludes `password`, `secret`, `token` columns by convention)
   - `UserWithExtrasDto { #[serde(flatten)] base: UserDto, is_admin: bool, ... }` — computed fields from accessors (`#[rok(accessor)]` on model)
2. **Date serialization**:
   - Global `#[rok(serialize_date = "format")]` on model → all `DateTime<Utc>`/`NaiveDateTime` fields use this `serde` format unless overridden
   - Per-field `#[rok(cast(created_at = "datetime:Y-m-d"))]` overrides global
4. **No runtime hidden/visible/appends** — DTO selection is the mechanism. `make_visible`/`set_hidden` not needed.

**Blocked by:** 05 (casting system provides the `cast` attribute for date formats).

**Status:** ready-for-agent

- [ ] Generator emits three DTOs per model when accessors exist or sensitive columns detected
- [ ] `UserPublicDto` excludes columns named `password`, `secret`, `token`, `api_key`, `private_key` (configurable via `#[rok(hidden_columns = [...])]`)
- [ ] `UserWithExtrasDto` includes `#[serde(flatten)] base: UserDto` plus accessor return types
- [ ] `serialize_date` derive attribute → `#[serde(with = "...")]` on all date fields
- [ ] Per-field format in `cast` attribute overrides global
- [ ] Facade tests: DTO variants, date formats
- [ ] Module fixture expectations for generated DTOs