# 02: Generate relations on the model (belongs_to / has_many / has_one)

**What to build:** Foreign keys discovered in the schema become `#[rok(belongs_to = Target)]` on the child model and `#[rok(has_many(children = Child::FK))]` / `#[rok(has_one(child = Child::FK))]` on the parent model. The generated model struct gains typed relation constants (`User::POSTS`, `Post::AUTHOR`) and accessor methods (`user.posts()`, `post.author()`) so call sites read `user.posts().all(&db).await?` instead of `relationships::load_posts(&db, &users).await?`.

**Blocked by:** 01 (the generator's write surface must be stable so relation accessors don't collide with write verbs).

**Status:** ready-for-agent

- [ ] Schema parser's `Relationship` analysis feeds the model emitter: for each FK, emit `belongs_to` on the holding table; for each referenced table, emit `has_many` (or `has_one` when the FK has a unique constraint)
- [ ] Generated model has `pub const RELATION_NAME: BelongsTo<Self, Target>` / `HasMany<Self, Target>` / `HasOne<Self, Target>` constants and `pub fn relation_name(&self) -> Select<Target>` accessors
- [ ] Relation names follow the derive's existing rule: `user_id` → `user`; ambiguous names use explicit form `belongs_to(author = User)`
- [ ] The per-table `relationships.rs` module is retained for its loader functions (batch loads), but generated code uses the model accessors where possible
- [ ] Soft-deleted related rows are hidden by default (the accessor's base query includes the soft-delete scope)
- [ ] Templates, fixtures, and `examples/api` regenerate with relations on the model; hand-written service code can call `user.posts()` directly
- [ ] `rok gen check` passes with no drift; template CI compiles under strict lints