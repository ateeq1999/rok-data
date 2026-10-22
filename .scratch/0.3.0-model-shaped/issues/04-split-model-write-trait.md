# 04: Split Model write verbs into a separate trait (fix large-file gate)

**What to build:** The `Model` trait in `row.rs` exceeds the 600-line gate. The 8 write verbs (`insert`, `update`, `upsert`, `upsert_on`, `delete`, `force_delete`, `restore`, `update_only`) move to a new `ModelWrite` trait (or stay as inherent methods in `writes.rs` and the trait re-exports them). The `Model` trait stays the read/query surface; `ModelWrite` is the write surface. Both are in the prelude so existing imports don't break.

**Blocked by:** 01, 02 (write verbs must be stable before extracting them; relations on model may add more methods).

**Status:** done

- [x] New trait `ModelWrite` in `crates/rok-data-core/src/model/write.rs` containing the 8 write verbs + `update_only`
- [x] `Model` trait no longer declares the write verbs; it only has read/query verbs
- [x] Both traits are re-exported from `rok_data::prelude` so `use rok_data::prelude::*;` still brings both into scope
- [x] The derive's `expand.rs` implements `ModelWrite` for generated models (or relies on the inherent methods in `writes.rs` which already exist)
- [x] Hand-written models that currently `impl Model for MyStruct` get a blanket impl or must also `impl ModelWrite` — choose the path that minimizes user churn (blanket impl if all `Model` methods are present)
- [x] All facade tests, engine templates, fixtures, and `examples/api` compile and pass `cargo test --workspace --all-targets`
- [x] `row.rs` drops below 600 lines; `cargo clippy -D warnings` clean