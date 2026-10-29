import { Link, createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/about')({
  component: About,
})

function About() {
  return (
    <main className="page-wrap px-4 py-12">
      <section className="island-shell rounded-2xl p-6 sm:p-8">
        <p className="island-kicker mb-2">About</p>
        <h1 className="display-title gradient-text mb-3 text-4xl font-bold sm:text-5xl">
          One workspace, one truth.
        </h1>
        <p className="m-0 max-w-3xl text-base leading-8 text-[var(--sea-ink-soft)]">
          rok-data is what happens when four sibling repositories — rok-db,
          rok-fluent, rok-orm and rok-sqlc — stop duplicating each other.
          One runtime (typed models over sqlx 0.9, PostgreSQL-only), two
          generators, one CLI ({' '}
          <code>rok</code>), and one source of truth: the schema directory.
        </p>
      </section>

      <section className="island-shell mt-8 grid gap-6 md:grid-cols-2">
        <div className="rounded-2xl p-6">
          <p className="island-kicker mb-2">Lineage</p>
          <h2 className="mb-4 text-xl font-semibold text-[var(--sea-ink)]">
            Four origins, one shape
          </h2>
          <ul className="m-0 space-y-2 text-sm text-[var(--sea-ink-soft)]">
            <li><strong>rok-db</strong> — the runtime and its journal engine</li>
            <li><strong>rok-sqlc</strong> — the resource-module engine (absorbed, frozen: workspace ADR 0016)</li>
            <li><strong>rok-fluent</strong> — the fluent chain vocabulary</li>
            <li><strong>rok-orm</strong> — the model-shaped verbs' design</li>
          </ul>
          <p className="mt-4 text-sm text-[var(--sea-ink-soft)]">
            Everything the consolidation kept, changed or dropped is in the
            repository's docs —{' '}
            <a
              href="https://github.com/ateeq1999/rok-data/blob/main/docs/migration-guide.md"
              target="_blank"
              rel="noreferrer"
              className="hover:underline"
            >
              migration-guide.md
            </a>{' '}
            and{' '}
            <a
              href="https://github.com/ateeq1999/rok-data/tree/main/docs/decisions"
              target="_blank"
              rel="noreferrer"
              className="hover:underline"
            >
              workspace ADRs
            </a>
            .
          </p>
        </div>

        <div className="rounded-2xl p-6">
          <p className="island-kicker mb-2">Project</p>
          <h2 className="mb-4 text-xl font-semibold text-[var(--sea-ink)]">
            Facts
          </h2>
          <ul className="m-0 space-y-2 text-sm text-[var(--sea-ink-soft)]">
            <li>License: MIT OR Apache-2.0</li>
            <li>Rust ≥ 1.94, edition 2024; PostgreSQL 12+ only</li>
            <li>Crates on crates.io: rok-data, rok-data-core,
              rok-data-macros, rok-data-codegen, rok-data-sqlc,
              rok-data-cli</li>
            <li>The `rok` binary name is a deployed surface — it does not
              follow crate renames</li>
          </ul>
        </div>
      </section>

      <section className="island-shell mt-8 rounded-2xl p-6">
        <p className="island-kicker mb-2">Start</p>
        <h2 className="mb-4 text-xl font-semibold text-[var(--sea-ink)]">
          The front door, again
        </h2>
        <pre className="text-xs"><code>{`$ cargo install rok-data-cli
$ rok init my-api --schema schema.sql
$ cd my-api && rok gen`}</code></pre>
        <p className="mt-4 m-0 text-sm text-[var(--sea-ink-soft)]">
          From here:{' '}
          <Link to="/guide/getting-started" className="hover:underline">getting started</Link>,{' '}
          <Link to="/crates/rok-cli" className="hover:underline">the CLI</Link>,{' '}
          <Link to="/agents" className="hover:underline">agents</Link>.
        </p>
      </section>
    </main>
  )
}
