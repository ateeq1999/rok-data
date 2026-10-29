import { Link, createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/crates/rok-data-codegen')({
  component: RokDataCodegenDocs,
})

function RokDataCodegenDocs() {
  return (
    <div className="mx-auto max-w-4xl px-4 py-8">
      <article className="prose dark:prose-invert max-w-none">
        <h1>rok-data-codegen</h1>
        <p className="text-lg text-[var(--sea-ink-soft)]">
          The whole-crate generator (legacy `rok generate`) and the
          migration diff engine behind `rok gen --migration`.
        </p>

        <div className="my-4 flex items-center gap-4">
          <a
            href="https://crates.io/crates/rok-data-codegen"
            target="_blank"
            rel="noreferrer"
            className="no-underline rounded-full bg-[var(--chip-bg)] border border-[var(--chip-line)] px-3 py-1.5 text-sm font-semibold"
          >
            crates.io
          </a>
          <a
            href="https://github.com/ateeq1999/rok-data/tree/main/crates/rok-data-codegen"
            target="_blank"
            rel="noreferrer"
            className="text-sm text-[var(--sea-ink-soft)] no-underline hover:underline"
          >
            source: crates/rok-data-codegen
          </a>
        </div>

        <p>
          Two jobs, one crate: parse <code>.sql</code> files into a Schema
          plus typed queries, and diff the schema over time into migration
          pairs. It is the library
          <code> rok_data_codegen::generate</code> behind the CLI.
        </p>

        <h2>The legacy CLI path</h2>
        <pre><code>{`$ mkdir sql
$ vi sql/decisions.sql      # your CREATE TABLE files, one or more
$ rok generate --input sql --output db-gen --crate-name my-app-db`}</code></pre>
        <p>
          Defaults come from <code>rok.toml</code>'s <code>[generate]</code>{' '}
          table (the legacy <code>rok-data.toml</code> name is still read,
          with a notice); <strong>flags beat the file</strong>.
        </p>
        <pre><code>{`[generate]
input = "sql"                       # .sql sources
output = "db-gen"                   # generated crate directory
crate_name = "notes-db"
rok_data_version = "0.3"            # or rok_data_path = "../" path dep`}</code></pre>
        <table className="w-full">
          <thead>
            <tr className="border-b">
              <th className="py-2 text-left">Flag</th>
              <th className="py-2 text-left">Meaning</th>
            </tr>
          </thead>
          <tbody>
            <tr className="border-b"><td className="py-2 font-mono text-sm">--migration NAME</td><td className="py-2 text-sm">append an auto-migration for changes since the last run</td></tr>
            <tr className="border-b"><td className="py-2 font-mono text-sm">--allow-destructive</td><td className="py-2 text-sm">allow steps that lose data or fail existing rows</td></tr>
            <tr className="border-b"><td className="py-2 font-mono text-sm">--rename OLD=NEW</td><td className="py-2 text-sm">table or column renames kept as identity (tbl or tbl.col)</td></tr>
            <tr className="border-b"><td className="py-2 font-mono text-sm">--strict</td><td className="py-2 text-sm">fail on generator warnings (no tenant/soft-delete filters)</td></tr>
          </tbody>
        </table>
        <pre><code>$ rok generate check --strict     # CI: exit 1 on drift</code></pre>

        <h2>What the emitted crate carries</h2>
        <ul>
          <li>A Cargo project to the requested name with the facade pinned
            (<code>rok_data_version</code> or <code>rok_data_path</code> —
            mutually exclusive)</li>
          <li>Model structs + typed column constants, chainable queries,
            write verbs taking generated inputs.</li>
          <li>The migration journal wired in — the generated migration
            constants embed the same pair text via{' '}
            <code>include_str!</code>, so checksums match{' '}
            <code>rok migrate</code>'s</li>
        </ul>

        <h2>Legacy note</h2>
        <p>
          <code>rok generate</code> keeps its flags, output and exit codes
          and is superseded by <code>rok init</code>/{' '}
          <code>rok gen</code> — removal is deferred to 1.0 (engine ADR
          0012). New projects should use the front door.
        </p>

        <h2>Links</h2>
        <ul>
          <li><Link to="/crates/rok-cli">the rok binary</Link></li>
          <li><Link to="/crates/rok-data-sqlc">rok-data-sqlc — the resource-module engine</Link></li>
        </ul>
      </article>
    </div>
  )
}
