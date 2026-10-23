# 01: Reconcile `main` with `origin/main`

**What to build:** A `main` that is no longer diverged, so every later ticket starts from one history. Local `main` is 6 commits ahead of origin (0.3.0 tickets, ModelCollection) and 1 behind (`cfe6c41`, the tasks scaffold). A trial merge conflicts in the 0.3.0 spec and in the engine's `milestone-1.txt` snapshot.

**Blocked by:** None (can start immediately), once the owner of the uncommitted `rok-data-core` edits (query sugar in progress) has committed or stashed them.

**Status:** ready-for-agent

- [ ] Uncommitted work in the tree is committed or stashed by its owner — not discarded
- [ ] `origin/main` merged (merge, not rebase: one conflict round instead of six)
- [ ] The 0.3.0 spec conflict resolved by hand
- [ ] `milestone-1.txt` regenerated with `ROK_DATA_SQLC_UPDATE_EXPECTED=1`, never hand-merged
- [ ] fmt, clippy `-D warnings`, `cargo test --workspace --all-targets`, module corpus and fixture compile gates green
