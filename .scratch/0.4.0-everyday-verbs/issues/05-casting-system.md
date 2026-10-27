# 05: Casting system — standard casts, custom casts, value objects, inbound-only, castables, parameters

**What to build:** A `Cast<M>` trait in `crates/rok-data-core/src/cast.rs` with `get(&M, &str) -> Value` and `set(&mut M, &str, Value) -> Vec<(&str, Value)>`. The derive expands `#[rok(casts(...))]` and `#[rok(cast(...))]` into field accessors/mutators that wire the caster.

Standard casts (inferred by generator from column type):
- `boolean` — `i16`/`int` ↔ `bool`
- `json` — `serde_json::Value` ↔ `String` (JSONB/JSON)
- `array` — `Vec<String>` ↔ `String` (text[])
- `datetime`/`date`/`timestamp` — `DateTime<Utc>`/`NaiveDate`/`NaiveDateTime` with optional format `datetime:Y-m-d`
- `encrypted` — `String` ↔ encrypted `String` (needs key source from config)
- `enum` — already works via `DbEnum`

Custom casts:
- `#[rok(cast(field = AsCustomCast:param1,param2))]` — user type implements `Cast<M>`; params passed to `AsCustomCast::with_params(&[&str])`
- Inbound-only: implement only `CastInbound` (has `set`, no `get`); derive skips getter
- Value object: `#[rok(cast(address = AsAddress))]` where `AsAddress::get(&M) -> Address`, `AsAddress::set(&mut M, Address) -> Vec<(&str, Value)>`
- Castable: `#[rok(cast(address = Address))]` where `Address: Castable` with `type Caster: Cast<Address>; fn caster() -> Self::Caster`

Generator: infers casts from column types (`BOOLEAN`→`boolean`, `JSONB`→`json`, `TIMESTAMPTZ`→`datetime`, `DATE`→`date`, `ENUM`→`enum`, `password` column→`encrypted` or inbound hash).

**Blocked by:** 01 (ModelCollection for fresh), 03 (value objects can be related models).

**Status:** ready-for-agent

- [ ] `Cast<M>`, `CastInbound<M>`, `Castable` traits in `cast.rs`
- [ ] Derive expansion for `casts` (repeatable field-level) and `cast` (single field with params)
- [ ] Standard caster implementations in `crates/rok-data-core/src/cast/standard.rs`
- [ ] Generator emits `casts` for standard column types; `password` → inbound hash or `encrypted`
- [ ] `serialize_date` global + per-field format via `datetime:format`
- [ ] Facade tests: each standard cast, custom cast, value object, inbound-only, castable, parameters
- [ ] Module fixture expectations for generated casts