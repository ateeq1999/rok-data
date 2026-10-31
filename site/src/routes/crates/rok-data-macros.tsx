import { Link, createFileRoute } from '@tanstack/react-router'
import { Badge } from '../../components/ui/badge'

export const Route = createFileRoute('/crates/rok-data-macros')({
  component: RokDataMacrosDocs,
})

function RokDataMacrosDocs() {
  return (
    <div className="mx-auto max-w-4xl px-4 py-8">
      <article className="prose max-w-none">
        <h1>rok-data-macros</h1>
        <p className="text-lg text-muted-foreground">
          The proc-macro crate: the Model derive, the row/value derives, and
          the #[rok_data::test] attribute.
        </p>

        <div className="not-prose my-4 flex items-center gap-4">
          <Badge
            variant="outline"
            render={<a href="https://crates.io/crates/rok-data-macros" target="_blank" rel="noreferrer" />}
          >
            crates.io
          </Badge>
          <a
            href="https://github.com/ateeq1999/rok-data/tree/main/crates/rok-data-macros"
            target="_blank"
            rel="noreferrer"
            className="text-sm text-muted-foreground no-underline hover:underline"
          >
            source: crates/rok-data-macros
          </a>
        </div>

        <p>
          Applications never depend on this crate directly — the{' '}
          <Link to="/crates/rok-data">rok-data</Link> facade re-exports
          everything. Derives can also be referenced as{' '}
          <code>rok_data::macros‑…</code> when needed explicitly.
        </p>

        <h2>derive(Model)</h2>
        <p>
          Expands a struct into the typed column constants, the relation
          and scope expansions (entries, chain methods, accessors) and the
          <code> Model</code>/<code>ModelWrite</code> implementations.
        </p>
        <pre><code>{`#[derive(Debug, Clone, Model)]
#[rok(table("note"), per_page = 20, tenant, soft_delete, version)]
#[rok(has_many(notes = Note::AUTHOR_ID))]
#[rok(belongs_to(author = User))]
#[rok(scopes(popular = popular))]
struct Note {
    #[rok(generated)]            // DB fills it — excluded on insert
    id: i64,
    #[rok(column("created_at"))] // name override
    created: OffsetDateTime,
    #[rok(skip)]                 // not persisted
    author_name: String,
}`}</code></pre>

        <h2>Attribute cheat sheet</h2>
        <table className="w-full">
          <thead>
            <tr className="border-b">
              <th className="py-2 text-left">Attribute</th>
              <th className="py-2 text-left">Meaning</th>
            </tr>
          </thead>
          <tbody>
            <tr className="border-b"><td className="py-2 font-mono text-sm">generated</td><td className="py-2 text-sm">the database fills it; excluded on insert</td></tr>
            <tr className="border-b"><td className="py-2 font-mono text-sm">primary_key</td><td className="py-2 text-sm">the alternate key column (composite in field order)</td></tr>
            <tr className="border-b"><td className="py-2 font-mono text-sm">has_many / has_one / belongs_to</td><td className="py-2 text-sm">relations: name = FK column const (or owner model)</td></tr>
            <tr className="border-b"><td className="py-2 font-mono text-sm">scopes(name = fn)</td><td className="py-2 text-sm">named scope; the fn becomes the entry</td></tr>
            <tr className="border-b"><td className="py-2 font-mono text-sm">per_page = N</td><td className="py-2 text-sm">the model's page size (15 by default)</td></tr>
            <tr className="border-b"><td className="py-2 font-mono text-sm">tenant / soft_delete / version</td><td className="py-2 text-sm">tenant scoping, the deleted_at pair, optimistic locking</td></tr>
            <tr className="border-b"><td className="py-2 font-mono text-sm">validate(…)</td><td className="py-2 text-sm">unique(…), non_empty, length(min…), range(min…), email — with database-backed rules</td></tr>
            <tr className="border-b"><td className="py-2 font-mono text-sm">column("name") / table("name") / skip</td><td className="py-2 text-sm">naming and persistence overrides</td></tr>
            <tr className="border-b"><td className="py-2 font-mono text-sm">default_scope(…) / nested / type_name(…)</td><td className="py-2 text-sm">a default filter, nested-row input, display naming</td></tr>
          </tbody>
        </table>

        <h2>The row/value derives</h2>
        <pre><code>{`#[derive(FromRow)]  // map a query row onto a struct
#[derive(DbEnum)]   // a Rust enum, stored in PostgreS, with its casts
#[derive(DbNewtype)]// a newtype with its column conversions`}</code></pre>

        <h2>#[rok_data::test]</h2>
        <p>
          Tests that touch the database: creates a schema-scoped{' '}
          <code>Db</code> per test (the original{' '}
          <code>#[rok_db::test]</code> shape), so fixtures and teardown are
          automatic.
        </p>
        <pre><code>{`#[rok_data::test]
async fn notes_are_counted(db: rok_data::Db) -> rok_data::Result<()> {
    Note { id: 0, … }.insert(&db).await?;
    assert_eq!(Note::count(&db).await?, 1);
    Ok(())
}`}</code></pre>
        <p>
          Without <code>DATABASE_URL</code> the database-backed suites skip
          honestly — printed, never silent.
        </p>

        <h2>Links</h2>
        <ul>
          <li><Link to="/crates/rok-data">rok-data — the facade</Link></li>
          <li><a href="https://github.com/ateeq1999/rok-data/blob/main/docs/getting-started.md" target="_blank" rel="noreferrer">docs/getting-started.md — the compiled samples</a></li>
        </ul>
      </article>
    </div>
  )
}
