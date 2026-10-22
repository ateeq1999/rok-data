# Toasty ORM Patterns Research

> **Source**: [tokio-rs/toasty](https://github.com/tokio-rs/toasty) (cloned at commit `HEAD` as of 2026-10-22)
> **Researcher**: OpenCode subagent
> **Purpose**: Reference implementation patterns for Rust ORM design

---

## 1. Overall Architecture

### Workspace Structure (Multi-crate)

```
toasty/
├── crates/
│   ├── toasty/                     # Main user-facing API
│   ├── toasty-core/                # Shared types: schema, AST, Driver trait, Error
│   ├── toasty-macros/              # Proc-macro: #[derive(Model)], #[derive(Embed)]
│   ├── toasty-sql/                 # SQL serializer (AST → SQL strings)
│   ├── toasty-cli/                 # CLI binary for migrations
│   └── toasty-driver-*             # Backend drivers (sqlite, postgresql, mysql, dynamodb, turso)
├── examples/                       # Example apps (each is a crate)
└── tests/                          # Integration test fixtures
```

**Key invariants**:
- Single binary: `toasty-cli` (not `toasty`)
- Drivers are optional features behind `feature = "sqlite"`, `"postgresql"`, etc.
- `toasty-core` is the only crate that depends on `sqlx-core` (for types only)
- Library crates never depend on CLI; CLI delegates to library functions

### Public API Surface (`toasty/src/lib.rs`)

```rust
pub mod db;        // Db, Connection, Pool, Executor, Transaction
pub mod schema;    // Model, Embed, Field, RelationOneField, RelationManyField, Deferred
pub mod stmt;      // Query, Insert, Update, Delete, Upsert, Expr, Path, Include
pub mod sql;       // Raw SQL execution helpers
pub mod migration; // Migration history, snapshots, generation
```

---

## 2. Model Definition

### Derive Macros

```rust
#[derive(Debug, toasty::Model)]
struct Author {
    #[key]           // Primary key
    #[auto]          // Auto-generated (UUID v7 for Uuid, serial for integers)
    id: uuid::Uuid,

    name: String,

    #[unique]        // Creates unique index + generates get_by_* method
    email: String,

    #[has_many]      // One-to-many (child owns the FK)
    posts: toasty::Deferred<Vec<Post>>,
}

#[derive(Debug, toasty::Model)]
struct Post {
    #[key]
    #[auto]
    id: uuid::Uuid,

    title: String,

    #[index]         // Creates index (not unique)
    author_id: uuid::Uuid,

    #[belongs_to]    // Many-to-one (this model owns the FK)
    author: toasty::Deferred<Author>,
}
```

### Key Attributes

| Attribute | Purpose | Generates |
|-----------|---------|-----------|
| `#[key]` | Primary key | PK column + `get_by_id` |
| `#[auto]` | Auto-generate value | UUID v7 / serial; omitted from create |
| `#[unique]` | Unique constraint | Unique index + `get_by_<field>` + `filter_by_<field>` |
| `#[index]` | Regular index | Non-unique index |
| `#[has_many]` | One-to-many relation | `Deferred<Vec<T>>` field + accessor |
| `#[has_one]` | One-to-one (parent side) | `Deferred<Option<T>>` + accessor |
| `#[belongs_to]` | Many-to-one (child side) | FK column + `Deferred<Option<T>>` + accessor |
| `#[has_many(via = ...)]` | Many-to-many via join | Read-only `Deferred<Vec<T>>` |

### Table/Column Inference Conventions

- Table name = snake_case of struct name (`Author` → `authors`)
- Column name = snake_case of field name (`author_id` → `author_id`)
- FK column = `<parent_snake>_id` (`author_id`) — auto-inferred from `belongs_to`
- Referenced key = parent's `#[key]` field — auto-inferred

---

## 3. Query Builder (Fluent API)

### Core Types

```rust
// Query<T> where T = returning shape
Query<List<User>>      // → Vec<User>
Query<User>            // → User (errors if not found)
Query<Option<User>>    // → Option<User>

// Built by:
User::all()                    // Query<List<User>>
User::filter_by_email("...")   // Query<List<User>>
User::get_by_id(&mut db, id)   // Query<User>  (executes immediately)
User::get_by_email(&mut db, e) // Query<User>  (executes immediately)
```

### Chainable Methods (on `Query<List<M>>`)

```rust
Query::<List<User>>::all()
    .filter(User::fields().age().gt(18))  // AND-combined
    .filter(User::fields().name().eq("Alice"))
    .order_by((User::fields().age().desc(), User::fields().name().asc()))
    .limit(10)
    .offset(20)
    .include(User::fields().posts())           // Eager load
    .include(User::fields().posts().filter(Post::fields().published().eq(true)))
```

### Terminal Methods

| Method | Returns | Behavior |
|--------|---------|----------|
| `.exec(&mut db).await` | `Vec<M>` | Execute list query |
| `.first().exec(&mut db).await` | `Option<M>` | LIMIT 1, returns `Some`/`None` |
| `.one().exec(&mut db).await` | `M` | LIMIT 1, errors if missing |
| `.get(&mut db).await` | `M` | Alias for `.one()` |
| `.count().exec(&mut db).await` | `u64` | `SELECT COUNT(*)` |
| `.select(proj).exec(&mut db).await` | `Vec<T>` | Projection (field/tuple) |
| `.delete().exec(&mut db).await` | `()` | Delete matching rows |

### Expression Building (Type-Safe)

```rust
// Path from model to field
User::fields().name()        // Path<User, String>
User::fields().age()         // Path<User, i64>
User::fields().posts()       // Path<User, List<Post>>

// Comparisons → Expr<bool>
path.eq("Alice")
path.ne("Bob")
path.gt(18)
path.ge(1)
path.lt(100)
path.le(100)
path.between(18, 65)
path.in_list([1, 2, 3])
path.in_query(subquery)

// Boolean combinators
expr.and(other)
expr.or(other)
!expr
Expr::and_all(iter)

// Nullable helpers
path.is_none()  // IS NULL
path.is_some()  // IS NOT NULL

// String ops
path.starts_with("Al")
path.like("Al%")
path.ilike("al%")           // PostgreSQL only
path.like_with_escape("Al\\%%", '\\')
```

---

## 4. Relationships

### BelongsTo / HasMany (Standard 1:N)

```rust
// Parent
#[derive(Model)]
struct User {
    #[key] #[auto] id: Uuid,
    #[has_many]
    posts: Deferred<Vec<Post>>,
}

// Child
#[derive(Model)]
struct Post {
    #[key] #[auto] id: Uuid,
    #[index] user_id: Uuid,           // FK column (inferred)
    #[belongs_to]
    user: Deferred<User>,             // References User::id
}
```

**Accessor patterns**:
- `user.posts()` → `Query<List<Post>>` (scoped to this user)
- `post.user()` → `Query<Option<User>>` (loads parent)

### HasOne / BelongsTo (1:1)

```rust
#[derive(Model)]
struct User {
    #[key] #[auto] id: Uuid,
    #[has_one]
    profile: Deferred<Option<Profile>>,
}

#[derive(Model)]
struct Profile {
    #[key] #[auto] id: Uuid,
    #[unique] user_id: Option<Uuid>,   // Unique FK = 1:1
    #[belongs_to]
    user: Deferred<Option<User>>,
}
```

**Key insight**: 1:1 is enforced by `#[unique]` on the FK, not a separate attribute.

### Many-to-Many (via Join Model)

```rust
#[derive(Model)]
struct User {
    #[key] #[auto] id: Uuid,
    #[has_many]
    memberships: Deferred<Vec<Membership>>,

    // Read-only traversal through join model
    #[has_many(via = memberships.group)]
    groups: Deferred<Vec<Group>>,
}

#[derive(Model)]
struct Group {
    #[key] #[auto] id: Uuid,
    #[has_many]
    memberships: Deferred<Vec<Membership>>,

    #[has_many(via = memberships.user)]
    users: Deferred<Vec<User>>,
}

#[derive(Model)]
struct Membership {
    #[key] #[auto] id: Uuid,
    #[index] user_id: Uuid,
    #[belongs_to] user: Deferred<User>,
    #[index] group_id: Uuid,
    #[belongs_to] group: Deferred<Group>,
}
```

**Patterns**:
- Direct field (`memberships`) = mutable, owns the join records
- `via` field (`groups`) = read-only, deduplicates targets
- Mutate via `user.memberships.insert(...)`, read via `user.groups()`

---

## 5. Eager Loading (`.include()`)

### Basic Include

```rust
// Load users WITH their posts in ONE round-trip
let users = User::all()
    .include(User::fields().posts())
    .exec(&mut db)
    .await?;

// Access loaded data WITHOUT .await (already in memory)
for user in &users {
    println!("{} has {} posts", user.name, user.posts.get().len());
    //            ^^^^^^^^^^^^^^^^^^^  .get() not .await!
}
```

### Filtered / Ordered Include

```rust
User::all()
    .include(
        User::fields()
            .posts()
            .filter(Post::fields().published().eq(true))  // Only published
            .order_by(Post::fields().created_at().desc()) // Order children
    )
    .exec(&mut db)
    .await?;
```

### Via Relations (Multi-step)

```rust
// Include threads Alice participated in (User -> comments -> thread)
User::all()
    .include(User::fields().participated_threads())
    .exec(&mut db)
    .await?;
```

**Supported backends**: SQL (SQLite, PostgreSQL, MySQL) — NOT DynamoDB.

---

## 6. Transactions

### Basic Transaction

```rust
let mut tx = db.transaction().await?;
User::create(...).exec(&mut tx).await?;
Post::create(...).exec(&mut tx).await?;
tx.commit().await?;  // Auto-rollback on drop if not committed
```

### TransactionBuilder (Configured)

```rust
let mut tx = db.transaction_builder()
    .isolation(IsolationLevel::Serializable)
    .read_only(false)
    .mode(TransactionMode::Default)
    .begin()
    .await?;
```

### Nested Transactions (Savepoints)

```rust
let mut tx = db.transaction().await?;
{
    let mut nested = tx.transaction().await?;  // Savepoint
    // ... operations ...
    nested.commit().await?;  // RELEASE SAVEPOINT
}
// Outer tx still open
tx.commit().await?;
```

### Isolation Levels (Driver-Dependent)

```rust
pub enum IsolationLevel {
    ReadUncommitted,
    ReadCommitted,
    RepeatableRead,
    Serializable,
    Snapshot,      // PostgreSQL / SQL Server
}
```

**Note**: PostgreSQL rejects `Immediate`/`Exclusive` modes (SQLite-specific).

---

## 7. Migrations

### Schema Push (Dev/Tests)

```rust
db.push_schema().await?;  // Creates tables from models directly
```

### Migration Workflow (Production)

```bash
# 1. Generate migration from model diff
cargo run --bin migrate -- migration generate --name add_user_bio

# 2. Review generated SQL in toasty/
# 3. Apply to database
cargo run --bin migrate -- migration apply
```

### Migration Files (`toasty/` directory)

```
toasty/
├── 0001_initial.toml      # TOML with metadata + SQL
├── 0002_add_user_bio.toml
└── snapshot.json          # Current schema snapshot
```

### Migration Generation Core (`toasty/src/migration/generate.rs`)

```rust
pub fn generate(
    driver: &dyn Driver,
    previous: &db::Schema,
    next: &db::Schema,
    rename_hints: &diff::RenameHints,
) -> Option<Generated> {
    let schema_diff = diff::Schema::from(previous, next, rename_hints);
    if schema_diff.is_empty() { return None; }
    Some(Generated {
        migration: driver.generate_migration(&schema_diff),
        snapshot: Snapshot::new(next.clone()),
    })
}
```

### Applied Migrations Table

```sql
CREATE TABLE __toasty_migrations (
    id BIGINT PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TIMESTAMP NOT NULL
);
```

---

## 8. Async Runtime & Connection Pooling

### Runtime: Tokio (Required)

```toml
tokio = { version = "1.50", features = ["macros", "rt", "sync", "time"] }
```

### Pool Configuration (`toasty/src/db/pool.rs`)

```rust
PoolConfig {
    max_size: cpu_cores * 2,           // Default
    timeouts: Default::default(),      // wait: 30s, create: 30s, recycle: 0s
    health_check_interval: Some(60s),  // Background sweep
    pre_ping: false,                   // Ping before checkout
    max_connection_lifetime: None,
    max_connection_idle_time: None,
}
```

### Pool Internals

- Uses `deadpool` with `Runtime::Tokio1`
- Each connection runs a **background task** (`ConnectionHandle`) for async exec
- Health check: pings longest-idle connection periodically
- On ping failure → escalate to full idle-pool sweep
- User-observed `connection_lost` → immediate sweep escalation

### Connection Lifecycle

```
Db::connection() → Pool::get() → Manager::create() → Driver::connect()
                                    ↓
                              ConnectionHandle::spawn()
                                    ↓
                              Background task processes ops
```

---

## 9. Type Safety

### Column → Rust Type Mapping

| SQL Type | Rust Type | Feature |
|----------|-----------|---------|
| UUID | `uuid::Uuid` | default |
| TEXT/VARCHAR | `String` | default |
| INTEGER/BIGINT | `i64` / `i32` | default |
| BOOLEAN | `bool` | default |
| TIMESTAMP | `jiff::Timestamp` | `jiff` |
| DATE | `jiff::Date` | `jiff` |
| DECIMAL | `rust_decimal::Decimal` | `rust_decimal` |
| DECIMAL | `bigdecimal::BigDecimal` | `bigdecimal` |
| JSON/JSONB | `serde_json::Value` | `serde` |
| ENUM | Rust `enum` with `#[derive(Embed)]` | default |
| CIDR | `cidr::IpCidr` | `net` |
| MAC | `macaddr::MacAddr6/8` | `net` |

### Custom Types

```rust
// Newtype wrapper with Auto promotion
#[derive(Embed)]
struct UserId(Uuid);  // Implements NewtypeOf<Inner = Uuid>

// Auto derives for newtype if inner is Auto
impl Auto for UserId {}  // Blanket impl via NewtypeOf
```

### Load Trait (Deserialization)

```rust
pub trait Load {
    type Output;
    fn ty() -> stmt::Type;
    fn load(value: stmt::Value) -> Result<Self::Output>;
    fn reload(target: &mut Self, value: stmt::Value) -> Result<()>;
}
```

---

## 10. Error Handling

### Error Type (`toasty-core/src/error.rs`)

```rust
#[derive(Clone)]
pub struct Error { inner: Arc<ErrorInner> }

pub enum ErrorKind {
    Adhoc,
    DriverOperationFailed,
    ConnectionLost,
    ConnectionPool,
    ExpressionEvaluationFailed,
    InvalidConnectionUrl,
    InvalidDriverConfiguration,
    InvalidTypeConversion,
    InvalidRecordCount,
    RecordNotFound,
    InvalidResult,
    InvalidSchema,
    InvalidStatement,
    ReadOnlyTransaction,
    SerializationFailure,      // Retryable
    TransactionTimeout,
    UnsupportedFeature,
    ValidationFailed,
    ConditionFailed,           // Optimistic lock mismatch
}
```

### Key Patterns

- **Cheap to clone**: Single `Arc` pointer
- **Error chaining**: `.context(other_err)` builds cause chain
- **Predicates**: `err.is_record_not_found()`, `err.is_connection_lost()`, etc.
- **Result alias**: `pub type Result<T> = std::result::Result<T, Error>;`

### Serialization Failure (Retryable)

```rust
// PostgreSQL error code 40001
Error::serialization_failure("concurrent update conflict")
```

---

## 11. Unique / Interesting Patterns

### 1. **Deferred Loading as Type Wrapper**

```rust
// Eager: data loaded with parent
posts: Vec<Post>,

// Lazy: query executed on access
posts: Deferred<Vec<Post>>,
```

The `Deferred<T>` wrapper controls loading strategy at **compile time** via the type system.

### 2. **GAT-Based Query Shapes**

```rust
trait Model {
    type Query<T>;  // Generic Associated Type
    // Query<List<Self>>, Query<Self>, Query<Option<Self>>
}
```

Same query builder type, different return shapes based on terminal method.

### 3. **Path-Chaining for Relations**

```rust
// Chain paths at type level
User::path_root()           // Path<User, User>
    .chain(User::path_field::<Post>(2))  // Path<User, List<Post>>
    .chain(Post::path_field::<String>(1)) // Path<User, String> (post.title)
```

### 4. **Association Filters (Subquery-Based)**

```rust
// Users who have ANY comment in async_thread
User::filter(
    User::fields().comments().any(
        Comment::fields().thread_id().eq(async_thread.id)
    )
)
```

Translates to `EXISTS (SELECT 1 FROM comments WHERE ...)` or `IN` subquery.

### 5. **Nested Creation (Builder Syntax)**

```rust
create!(Author {
    name: "Bob",
    email: "bob@example.com",
    posts: [{ title: "First" }, { title: "Second" }],
})
```

Children created in same statement, FKs auto-filled.

### 6. **Scoped Relation Mutations**

```rust
// Insert into relation (sets FK automatically)
user.posts().insert(Post { title: "New" }).exec(&mut db).await?;

// Remove from relation
user.posts().remove(post).exec(&mut db).await?;

// Update via builder
user.update()
    .posts([{ title: "New post" }])  // Creates + attaches
    .exec(&mut db).await?;
```

### 7. **Embedded Types (Flattened into Parent Table)**

```rust
#[derive(Embed)]
struct Address {
    street: String,
    city: String,
    zip: String,
}

#[derive(Model)]
struct User {
    #[key] #[auto] id: Uuid,
    address: Address,  // Fields flattened: address_street, address_city, address_zip
}
```

### 8. **Document Storage (JSON Column)**

```rust
#[derive(Model)]
struct User {
    #[key] #[auto] id: Uuid,
    #[document]  // Stored as JSON/JSONB
    metadata: UserMetadata,
}
```

### 9. **Statement AST + Driver Serialization**

- User builds typed `Query`/`Insert`/`Update`
- Converted to untyped `stmt::Statement` (AST)
- `toasty-sql` serializes AST → SQL string per dialect
- Driver executes parameterized query

### 10. **Inventory-Based Model Registration**

```rust
// In generated code:
inventory::submit! { DiscoverItem::Model(|| User::schema()) }

// At runtime:
ModelSet::discover()  // Collects all submitted models
```

Uses `inventory` crate for zero-boilerplate model discovery.

### 11. **Version Compatibility Checks (Compile-Time)**

Generated code includes compile-time checks:
- Storage compatibility (column type matches Rust type)
- Column type requirements
- Auto compatibility
- Version compatibility
- Indexable checks

Fails at compile time if schema drift detected.

---

## Summary: Patterns Worth Borrowing for rok-data

| Pattern | Why It's Good |
|---------|---------------|
| **Deferred<T> for lazy/eager** | Compile-time loading strategy |
| **GAT Query<T> shapes** | Type-safe terminal methods |
| **Path-based field access** | Composable, type-safe queries |
| **Include with filter/order** | Flexible eager loading |
| **Via relations** | Clean many-to-many without join entity in domain |
| **Scoped relation mutations** | FK management automatic |
| **Inventory model registration** | Zero-boilerplate discovery |
| **Compile-time version checks** | Catch drift early |
| **Background connection task** | Async exec without blocking pool |
| **Error chaining with predicates** | Ergonomic error handling |
| **Transaction builder pattern** | Configurable isolation/mode |
| **Migration as diff → SQL** | Declarative schema evolution |