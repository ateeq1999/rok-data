import { Link, createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/guide/getting-started')({
  component: GettingStarted,
})

function GettingStarted() {
  return (
    <div className="mx-auto max-w-4xl px-4 py-8">
      <article className="prose dark:prose-invert max-w-none">
        <h1>Getting started</h1>
        <p className="lead">
          The project's schema directory is the only source of truth: the
          plain <code>*.sql</code> files in it are the schema as it is now,
          and the staged <code>NNNN_name</code> pairs beside them are the
          history.
        </p>

        <h2>The front door: <code>rok init</code> and <code>rok gen</code></h2>
        <pre><code>{`$ rok init my-api --schema schema.sql
$ cd my-api
$ rok gen`}</code></pre>
        <p>
          <code>rok init</code> writes the application skeleton
          (<code>src/main.rs</code>, <code>src/app.rs</code>,{' '}
          <code>src/config.rs</code>, <code>src/error.rs</code>,{' '}
          <code>src/openapi.rs</code>, <code>src/shared/</code>), the
          settings file, an <code>AGENTS.md</code> and the schema
          directory, seeded with the schema file you named. Every file it
          writes is written <strong>once and never rewritten</strong>.
        </p>
        <p>
          <code>rok gen</code> reads the schema directory and writes the
          data layer (<code>src/db/**</code>), the shared tools and the API
          features (<code>src/features/**</code>), printing every file.
        </p>

        <h2>Stage a schema change as a migration</h2>
        <pre><code>$ rok gen --migration add_posts</code></pre>
        <p>
          That writes <code>0002_add_posts.up.sql</code> and its
          <code>.down.sql</code> — the difference between the schema and
          the last staged pair. Destructive steps refuse until{' '}
          <code>--allow-destructive</code>; <code>--rename old=new</code>{' '}
          keeps renames instead of drop+create.
        </p>

        <h2>CI mode</h2>
        <pre><code>$ rok gen check</code></pre>
        <p>
          Writes nothing, reports missing/changed/stale files and{' '}
          <strong>exits 1</strong> when the generated code is out of date —
          the drift gate and the agent's freshness oracle.
        </p>

        <h2>A whole API in one step</h2>
        <pre><code>{`$ rok init my-api --template axum    # a complete axum application
$ rok init my-api --template actix   # its actix-web twin
$ cd my-api && rok gen`}</code></pre>
        <p>
          The templates carry their own <code>users</code> and{' '}
          <code>audit-log</code> features, an OpenAPI document and tests —
          all pinned to <code>rok sqlc check-arch</code>'s layout rules.
        </p>

        <h2>By hand: the runtime</h2>
        <pre><code>{`$ cargo new notes && cd notes && cargo add rok-data
$ export DATABASE_URL=postgres://postgres:postgres@localhost:5432/notes`}</code></pre>
        <p>Write the pair <code>migrations/0001_create_notes.up.sql</code>:</p>
        <pre><code>{`CREATE TABLE notes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    text TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);`}</code></pre>
        <p>…and <code>0001_create_notes.down.sql</code>:</p>
        <pre><code>DROP TABLE notes;</code></pre>
        <pre><code>{`$ rok migrate status     # journal state per file
$ rok migrate up         # apply, transaction + advisory lock

$ rok --version          # the only binary: rok`}</code></pre>

        <h2>Derive the models</h2>
        <pre><code>{`use rok_data::prelude::*;

#[derive(Debug, Clone, Model)]
#[rok(has_many(notes = Note::AUTHOR_ID))]
struct User {
    #[rok(generated)]
    id: i64,
    name: String,
}

#[derive(Debug, Clone, Model)]
#[rok(scopes(popular = popular))]
struct Note {
    #[rok(generated)]
    id: i64,
    #[rok(belongs_to(author = User))]
    author_id: i64,
    text: String,
    views: i32,
}

fn popular(query: Select<Note>) -> Select<Note> {
    query.filter(Note::VIEWS.gt(100))
}`}</code></pre>
        <pre><code>{`// reads: one row by key, the rest as a chain
let most_read = Note::filter(Note::VIEWS.gt(100))
    .order_by(Note::VIEWS.desc())
    .all(&db)
    .await?;

// writes: the row-level verbs of ModelWrite
let stored = draft.insert(&db).await?;
stored.delete(&db).await?;

// relations: an accessor starts the related model's chain
let author = note.author().first(&db).await?;

// named scopes: #[rok(scopes(popular = popular))]
let popular = Note::popular().all(&db).await?;`}</code></pre>

        <h2>The legacy generators</h2>
        <p>
          <code>rok generate</code> (whole crate from <code>.sql</code>{' '}
          files) and <code>rok sqlc g</code> (resource modules into an
          existing crate) still work exactly as they did — flags, output
          and exit codes — but they are <strong>legacy</strong>: superseded
          by the front door, removed only at 1.0.
        </p>

        <h2>Read further</h2>
        <ul>
          <li><Link to="/crates/rok-cli">CLI reference — every command and flag</Link></li>
          <li><Link to="/crates/rok-data">rok-data — the facade crate</Link></li>
          <li><Link to="/agents">Coding agents — drive it unattended</Link></li>
        </ul>
      </article>
    </div>
  )
}
