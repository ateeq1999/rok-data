import { Link, createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/crates/rok-data-core')({
  component: RokDataCoreDocs,
})

function RokDataCoreDocs() {
  return (
    <div className="mx-auto max-w-4xl px-4 py-8">
      <article className="prose dark:prose-invert max-w-none">
        <h1>rok-data-core</h1>
        <p className="text-lg text-muted">
          The runtime: sqlx 0.9 over PostgreSQL only — the Db, the typed
          builder, the migration engine, introspection.
        </p>

        <div className="my-4 flex items-center gap-4">
          <a
            href="https://crates.io/crates/rok-data-core"
            target="_blank"
            rel="noreferrer"
            className="no-underline rounded-full bg-[var(--chip-bg)] border border-[var(--chip-line)] px-3 py-1.5 text-sm font-semibold"
          >
            crates.io
          </a>
          <a
            href="https://github.com/ateeq1999/rok-data/tree/main/crates/rok-data-core"
            target="_blank"
            rel="noreferrer"
            className="text-sm text-muted-foreground no-underline hover:underline"
          >
            source: crates/rok-data-core
          </a>
        </div>

        <p>
          The only place <code>sqlx</code> is depended on directly.
          Applications take the <code>rok-data</code> facade instead of this
          crate; generators and tools consume its types.
        </p>

        <h2>What lives here</h2>
        <table className="w-full">
          <thead>
            <tr className="border-b">
              <th className="py-2 text-left">Module</th>
              <th className="py-2 text-left">One-liner</th>
            </tr>
          </thead>
          <tbody>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">db, cache, context</td>
              <td className="py-2 text-sm">`Db`, pool stats, query cache, the `Executor` surface, tx options</td>
            </tr>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">expr, select, join</td>
              <td className="py-2 text-sm">the typed `Expr`/`Select` builder — conditions, casts, render</td>
            </tr>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">relation, collection</td>
              <td className="py-2 text-sm">`HasMany`/`HasOne`/`BelongsTo`, `ModelCollection`, With-loading</td>
            </tr>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">migration, migration_file</td>
              <td className="py-2 text-sm">journal engine (`_rok_data_migrations`) + the pair-file loader</td>
            </tr>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">introspect</td>
              <td className="py-2 text-sm">information_schema/pg_catalog reader.</td>
            </tr>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">wire, cursor</td>
              <td className="py-2 text-sm">`Page`/`Paginator`/`Resource` JSON shapes, cursor round-trips</td>
            </tr>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">tenant, validate, audit, encrypted</td>
              <td className="py-2 text-sm">tenant scoping, validation, audit history, encrypted columns</td>
            </tr>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">metrics, notify, testing</td>
              <td className="py-2 text-sm">gated metrics, change notifications, test helpers</td>
            </tr>
          </tbody>
        </table>

        <h2>The chain</h2>
        <pre><code>{`// conditions stay typed on the column
let found = Note::query()
    .filter_any([Note::TEXT], |column| column.ilike("%rust%"))
    .filter(Note::VIEWS.gt_column(Note::AUTHOR_ID))
    .get(&db)
    .await?;

// relations filter, count beside the rows, nest what they load
let writers = User::query().has(User::NOTES).all(&db).await?;
let view = User::query()
    .with_count(User::NOTES)
    .with(User::NOTES.with(Note::AUTHOR))
    .all(&db)
    .await?;`}</code></pre>

        <h2>Paging</h2>
        <table className="w-full">
          <thead>
            <tr className="border-b">
              <th className="py-2 text-left">Call</th>
              <th className="py-2 text-left">Returns</th>
            </tr>
          </thead>
          <tbody>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">paginate(db, page, per)</td>
              <td className="py-2 text-sm">`Page`: items, total, numbers</td>
            </tr>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">simple_paginate(db, page, per)</td>
              <td className="py-2 text-sm">`SimplePage`: items, has_more — no COUNT</td>
            </tr>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">cursor_paginate(db, cursor, per)</td>
              <td className="py-2 text-sm">`CursorPage`: stable under inserts</td>
            </tr>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">chunk_by_id(db, n, f) / lazy_by_id(db, n)</td>
              <td className="py-2 text-sm">walk a whole table / stream it</td>
            </tr>
          </tbody>
        </table>
        <p>
          Full paging reference on the{' '}
          <a
            href="https://github.com/ateeq1999/rok-data/blob/main/docs/pagination.md"
            target="_blank"
            rel="noreferrer"
          >
            docs/pagination.md
          </a>{' '}
          page.
        </p>

        <h2>The migration engine</h2>
        <pre><code>{`use rok_data::migration_file;

// load the pairs from a directory, journal-checked
let migrations = migration_file::load(std::path::Path::new("migrations"))?;
rok_data::migration::apply(&db, &migrations).await?;   // pending, oldest first
rok_data::migration::revert(&db, &migrations, 1).await?;`}</code></pre>
        <p>
          One transaction per migration, an advisory lock against two
          racing processes, checksums (FNV-1a of the <code>up</code> text)
          in <code>_rok_data_migrations</code> — running refuses on an
          edited applied migration, an unknown database version, or a
          pending migration older than the latest applied.
        </p>

        <h2>Links</h2>
        <ul>
          <li><Link to="/crates/rok-data">rok-data — the facade</Link></li>
          <li><Link to="/crates/rok-data-macros">rok-data-macros — the derives</Link></li>
          <li><a href="https://github.com/ateeq1999/rok-data/blob/main/docs/architecture.md" target="_blank" rel="noreferrer">docs/architecture.md</a></li>
        </ul>
      </article>
    </div>
  )
}
