import { Link, createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/crates/rok-data')({
  component: RokDataFacadeDocs,
})

function RokDataFacadeDocs() {
  return (
    <div className="mx-auto max-w-4xl px-4 py-8">
      <article className="prose dark:prose-invert max-w-none">
        <h1>rok-data</h1>
        <p className="text-lg text-[var(--sea-ink-soft)]">
          The facade: one crate that re-exports the runtime and the macros —
          the single dependency applications take.
        </p>

        <div className="my-4 flex items-center gap-4">
          <a
            href="https://crates.io/crates/rok-data"
            target="_blank"
            rel="noreferrer"
            className="no-underline rounded-full bg-[var(--chip-bg)] border border-[var(--chip-line)] px-3 py-1.5 text-sm font-semibold"
          >
            crates.io
          </a>
          <a
            href="https://github.com/ateeq1999/rok-data/tree/main/crates/rok-data"
            target="_blank"
            rel="noreferrer"
            className="text-sm text-[var(--sea-ink-soft)] no-underline hover:underline"
          >
            source: crates/rok-data
          </a>
        </div>

        <h2>Installation</h2>
        <pre><code>cargo add rok-data</code></pre>
        <p>
          Optional features ride the columns and tools you use:
          <code> chrono</code>, <code>uuid</code>, <code>decimal</code>,{' '}
          <code>json</code>, <code>migrate</code>, <code>serde</code>,{' '}
          <code>axum</code>, <code>metrics</code>, <code>ipnetwork</code>,{' '}
          <code>mac_address</code>. The default set covers the common
          shapes.
        </p>

        <h2>One import</h2>
        <pre><code>use rok_data::prelude::*;</code></pre>
        <p>
          The prelude brings the runtime's surface — the traits, builders
          and error type — plus the derive macros:
        </p>
        <table className="w-full">
          <thead>
            <tr className="border-b">
              <th className="py-2 text-left">What</th>
              <th className="py-2 text-left">Where it comes from</th>
            </tr>
          </thead>
          <tbody>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">Db, Tx, Executor</td>
              <td className="py-2 text-sm">connection, transactions, the execute surface</td>
            </tr>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">Select, Expr</td>
              <td className="py-2 text-sm">the typed query builder</td>
            </tr>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">Model / ModelWrite</td>
              <td className="py-2 text-sm">the verbs every derive(Model) struct carries</td>
            </tr>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">Model, FromRow, DbEnum, DbNewtype (derives)</td>
              <td className="py-2 text-sm">via rok-data-macros</td>
            </tr>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">Error, Result</td>
              <td className="py-2 text-sm">one error type (NotFound, conflicts, validation)</td>
            </tr>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">migration, migration_file</td>
              <td className="py-2 text-sm">the journal engine and the file loader</td>
            </tr>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">Page, CursorPage, LoadedView …</td>
              <td className="py-2 text-sm">paging, cursors and loaded relations</td>
            </tr>
          </tbody>
        </table>

        <h2>Quick start</h2>
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

// tests that touch the database get a schema-scoped Db
#[rok_data::test]
async fn popular_notes(db: rok_data::Db) -> rok_data::Result<()> {
    let most = Note::popular().all(&db).await?;
    assert!(!most.is_empty());
    Ok(())
}`}</code></pre>

        <h2>Links</h2>
        <ul>
          <li><Link to="/guide/getting-started">Getting started</Link></li>
          <li><Link to="/crates/rok-data-core">rok-data-core — the runtime this facade re-exports</Link></li>
          <li><Link to="/crates/rok-data-macros">rok-data-macros — the derives</Link></li>
          <li><a href="https://github.com/ateeq1999/rok-data/tree/main/docs" target="_blank" rel="noreferrer">Full docs (AI-friendly markdown)</a></li>
        </ul>
      </article>
    </div>
  )
}
