//! Derive macros for [rok-data](https://docs.rs/rok-data). Use them through the
//! `rok-data` crate rather than depending on this crate directly.

use proc_macro::TokenStream;
use syn::parse_macro_input;

mod attrs;
mod db_enum;
mod db_newtype;
mod model;
mod naming;
mod test;

/// Derive `rok_data::Model` (and `sqlx::FromRow`) for a struct with named fields.
///
/// # Container attributes
///
/// - `#[rok(table = "users")]` — table name, optionally schema-qualified.
///   Defaults to the struct name in `snake_case`, pluralised
///   (`User` → `users`, `BlogPost` → `blog_posts`, `Category` → `categories`).
/// - `#[rok(timestamps)]` — the `created_at` and `updated_at` fields are
///   managed automatically: both are set to `now()` on insert and
///   `updated_at` on every update.
/// - `#[rok(soft_delete)]` — the `deleted_at` field (an `Option` timestamp)
///   marks deleted rows: `delete` sets it, queries skip such rows unless
///   `with_trashed()`/`only_trashed()` is used, and `restore` clears it.
/// - `#[rok(has_many(posts = Post::USER_ID))]` — a one-to-many relation:
///   generates `User::POSTS` (a [`HasMany`]) and `user.posts()` (a query).
/// - `#[rok(has_one(profile = Profile::USER_ID))]` — a one-to-one relation:
///   generates `User::PROFILE` and `user.profile()`.
/// - `#[rok(has_many_through(posts = Post::USER_ID, through = User::COUNTRY_ID))]`
///   on `Country` — a relation through an intermediate model: generates
///   `Country::POSTS` (a `HasManyThrough`) and `country.posts()`;
///   `has_one_through(…)` is the one-target form.
/// - `#[rok(hooks)]` — you implement `rok_data::Hooks` yourself (otherwise a
///   no-op implementation is generated).
/// - `#[rok(validate_with = path::to::fn)]` — record-level validation,
///   `fn(&Self) -> Result<(), ValidationErrors>`, run with the field rules.
/// - `#[rok(default_scope = path::to::fn)]` — `fn() -> Expr<Self>` applied
///   to every query; remove it with `.unscoped()`.
/// - `#[rok(per_page = 25)]` — the rows a page holds when the caller names
///   no size (`Select::page`); 15 otherwise.
/// - `#[rok(no_from_row)]` — don't generate `FromRow` (bring your own).
/// - `#[rok(crate = "path::to::rok_data")]` — path of the `rok_data` crate when
///   it is re-exported under another name.
///
/// # Field attributes
///
/// - `#[rok(primary_key)]` — the primary key. Defaults to the field named `id`.
///   Mark several fields for a composite key (in field order); look records
///   up with a tuple: `Membership::find(&db, (org_id, user_id))`.
/// - `#[rok(generated)]` — filled in by the database (serial ids, defaults,
///   triggers): read, but never written by `insert`/`update`.
/// - `#[rok(column = "name")]` — column name, if it differs from the field.
/// - `#[rok(skip)]` — not a column; initialised with `Default::default()`.
/// - `#[rok(created_at)]` / `#[rok(updated_at)]` — managed timestamp column
///   with a custom name (see `timestamps`).
/// - `#[rok(deleted_at)]` — soft-delete column with a custom name.
/// - `#[rok(tenant)]` — the tenant column for row-level multi-tenancy
///   (see `rok_data::tenant`).
/// - `#[rok(version)]` — optimistic-locking counter (an integer): every
///   update increments it, and `update`/`delete` fail with a conflict error if
///   it changed since the record was loaded.
/// - `#[rok(validate(…))]` — validation rules checked before every insert
///   and update: `length(min = 1, max = 50)`, `range(min = 0, max = 150)`,
///   `email`, `non_empty`, `custom = path::to::fn` (`fn(&T) -> Result<(), String>`).
///   Rules on `Option` fields apply only to `Some`.
/// - `#[rok(belongs_to = User)]` on a foreign key field `user_id` — generates
///   `Post::USER` (a [`BelongsTo`]) and `post.user()`. Use
///   `#[rok(belongs_to(author = User))]` to pick the name.
///
/// [`HasMany`]: https://docs.rs/rok-data/latest/rok_data/struct.HasMany.html
/// [`BelongsTo`]: https://docs.rs/rok-data/latest/rok_data/struct.BelongsTo.html
///
/// # Generated items
///
/// Besides the trait impls, one typed column constant is generated per
/// column, named after the field in `SCREAMING_SNAKE_CASE`:
///
/// ```ignore
/// #[derive(Model)]
/// struct User { #[rok(primary_key, generated)] id: i64, email: String }
///
/// User::filter(User::EMAIL.eq("ann@example.com"))
/// ```
///
/// A relation whose name is a Rust keyword gets a raw-identifier accessor
/// (`r#type`), so the generated code compiles.
#[proc_macro_derive(Model, attributes(rok))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    model::expand::expand(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derive `sqlx::FromRow` for decoding query results into a plain struct,
/// e.g. the output of [`Select::select`] with aliased projections — without
/// depending on sqlx directly.
///
/// Fields are read by name; `#[rok(column = "name")]` and `#[rok(skip)]`
/// work as on [`Model`](derive@Model), and `#[rok(crate = "…")]` sets the
/// crate path.
///
/// ```ignore
/// #[derive(rok_data::FromRow)]
/// struct RoleStats { role: String, users: i64 }
/// ```
///
/// [`Select::select`]: https://docs.rs/rok-data/latest/rok_data/struct.Select.html#method.select
#[proc_macro_derive(FromRow, attributes(rok))]
pub fn derive_from_row(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    model::from_row::expand_from_row(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derive a column type for a field-less enum, stored as `TEXT` (default)
/// or as a native PostgreSQL enum.
///
/// - `#[rok(type_name = "mood")]` — use the PostgreSQL enum type `mood`
///   (`CREATE TYPE mood AS ENUM (…)`) instead of `TEXT`.
/// - `#[rok(rename_all = "snake_case")]` — how variant names are stored
///   (default `snake_case`; also `SCREAMING_SNAKE_CASE`, `kebab-case`,
///   `lowercase`, `UPPERCASE`, `PascalCase`).
/// - `#[rok(rename = "…")]` on a variant — explicit stored name.
///
/// Also generates `as_str()`, `Display` and `FromStr`, and makes the enum
/// usable in models, filters and `set` (it needs `Clone + Debug`).
///
/// ```ignore
/// #[derive(Debug, Clone, Copy, PartialEq, DbEnum)]
/// enum Role { Admin, Member, #[rok(rename = "ro")] ReadOnly }
///
/// User::filter(User::ROLE.eq(Role::Admin))
/// ```
#[proc_macro_derive(DbEnum, attributes(rok))]
pub fn derive_db_enum(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    db_enum::expand_db_enum(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derive a column type for a single-field tuple struct (a "newtype"),
/// stored exactly like its inner type:
///
/// ```ignore
/// #[derive(Debug, Clone, PartialEq, DbNewtype)]
/// struct Email(String);
///
/// #[derive(Model)]
/// struct User { id: i64, email: Email }
/// ```
///
/// The newtype must be `Clone + Debug`.
#[proc_macro_derive(DbNewtype, attributes(rok))]
pub fn derive_db_newtype(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    db_newtype::expand_db_newtype(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Run an async test against a fresh, temporary database (feature
/// `testing`). The database is created on the server named by
/// `DATABASE_URL`, passed in as a [`Db`], and dropped afterwards — even if
/// the test panics. Without `DATABASE_URL` the test is skipped.
///
/// ```ignore
/// #[rok_data::test(migrations = "migrations", sql = "tests/fixtures/users.sql")]
/// async fn finds_admins(db: Db) {
///     assert_eq!(User::filter(User::ROLE.eq("admin")).count(&db).await.unwrap(), 1);
/// }
/// ```
///
/// - `migrations = "dir"` — run sqlx migrations from `dir` (needs the
///   `migrate` feature); may be repeated.
/// - `sql = "file.sql"` — execute an SQL file (schema or fixtures); may be
///   repeated, runs after migrations in order.
///
/// Paths are relative to the crate's `Cargo.toml`. The test runs on
/// `#[tokio::test]`, so `tokio` (with `macros` and `rt`) must be a
/// dev-dependency. It may return `()` or a `Result<(), E>`.
///
/// [`Db`]: https://docs.rs/rok-data/latest/rok_data/struct.Db.html
#[proc_macro_attribute]
pub fn test(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as proc_macro2::TokenStream);
    let item = parse_macro_input!(item as syn::ItemFn);
    test::expand_test(args, item)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[cfg(test)]
mod tests;
