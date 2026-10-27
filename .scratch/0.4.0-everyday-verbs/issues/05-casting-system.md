# 05: Casting system — standard casts, custom casts, value objects, inbound-only, castables, parameters

**What to build (as specified):** a `Cast<M>` trait and `#[rok(casts(...))]` / `#[rok(cast(...))]` attributes: standard casts (`boolean`, `json`, `array`, `datetime`/`date`/`timestamp` with formats, `encrypted`, `enum`), custom casts with parameters, inbound-only casts, value objects and castables; the generator inferring casts from column types.

**Blocked by:** 01, 03.

**Status:** done (2026-10-27) — narrowed by decision with the user to what Rust types don't already give

**Decision.** A Rust field's type already is its cast: `bool`, `serde_json::Value`, `Vec<String>`, chrono's types and `DbEnum` decode straight from their columns, checked by the compiler, and the generator already picks them from the column types. A cast layer on top would duplicate that untyped. What types don't give, and what was built:

- [x] **Encrypted columns:** `Encrypted` (feature `encryption`), a text column stored as `base64(nonce ‖ AES-256-GCM ciphertext)`; the key from `set_encryption_key` or `ROK_DATA_ENCRYPTION_KEY`; `Debug` hides it (`crates/rok-data-core/src/encrypted.rs`, test `tests/features/encrypted.rs`)
- [x] **Value objects over several columns:** `#[rok(value_object(address: Address = (street, city)))]` generates `address()` / `set_address(…)`, through `From` / `Into` the fields' tuple (`tests/features/value_objects.rs`)
- [x] **Write-time transforms** (the spec's inbound-only cast): field attribute `#[rok(set_with = path)]` generates `set_<field>(value)` storing `path(value)`
- [x] Standard casts, custom casts with parameters, castables, generator-inferred casts — **dropped by decision** (the field type is the cast)
- [x] Date formats — **dropped by decision:** serde's own field attributes (`#[serde(with = …)]`) pass through the derive (ticket 07)
