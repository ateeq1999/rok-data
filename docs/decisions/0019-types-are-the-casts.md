# ADR 0019: a field's type is its cast

Date: 2026-10-27 · Status: accepted

## Context

The 0.4.0 spec asked for a casting layer: `#[rok(casts(flag = "boolean",
options = "json", published_at = "datetime:Y-m-d", secret = "encrypted"))]`,
custom casts with parameters, castables, generated DTO variants by column
name and per-field date formats.

In Rust, a field's type already decides how its column decodes and
encodes: `bool`, `serde_json::Value`, `Vec<String>`, chrono's types and
`DbEnum` types do so through sqlx, checked by the compiler, and the
generator already picks them from the column types. A string-named cast
over them would be a second, unchecked way to say the same thing.

## Decision

Only what a type can't say is added (decided with the user, 2026-10-27):

- **`Encrypted`** (feature `encryption`): a text column stored as
  `base64(nonce ‖ AES-256-GCM ciphertext)`, opened on read; the key is set
  once (`set_encryption_key`) or read from `ROK_DATA_ENCRYPTION_KEY`.
- **Value objects over several columns**:
  `#[rok(value_object(address: Address = (street, city)))]` generates
  `address()` / `set_address(…)` through `From` / `Into` the fields' tuple.
- **Write-time transforms**: `#[rok(set_with = hash)]` on a field
  generates `set_<field>(value)` storing `hash(value)` — the spec's
  inbound-only cast, as an explicit setter.

Not built: standard and custom casts, castables, cast parameters,
`serialize_date` (serde's own field attributes cover formats), and
convention-named public DTOs (the generated API's response DTOs already
leave out the table's `[api.hidden]` columns, an explicit list).

## Consequences

- The model stays the schema's shape in plain types; nothing between the
  column and the field can disagree with the field's type.
- An encrypted column can't be filtered or indexed by value (each write
  uses a fresh nonce).
