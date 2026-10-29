import { Link, createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/crates/rok-data-sqlc')({
  component: RokDataSqlcDocs,
})

function RokDataSqlcDocs() {
  return (
    <div className="mx-auto max-w-4xl px-4 py-8">
      <article className="prose dark:prose-invert max-w-none">
        <h1>rok-data-sqlc</h1>
        <p className="text-lg text-muted">
          The resource-module engine: parses a schema without a database and
          generates modules — models, queries, API features, OpenAPI — into
          an existing crate.
        </p>

        <div className="my-4 flex items-center gap-4">
          <a
            href="https://crates.io/crates/rok-data-sqlc"
            target="_blank"
            rel="noreferrer"
            className="no-underline rounded-full bg-[var(--chip-bg)] border border-[var(--chip-line)] px-3 py-1.5 text-sm font-semibold"
          >
            crates.io
          </a>
          <a
            href="https://github.com/ateeq1999/rok-data/tree/main/crates/rok-data-sqlc"
            target="_blank"
            rel="noreferrer"
            className="text-sm text-muted-foreground no-underline hover:underline"
          >
            source: crates/rok-data-sqlc
          </a>
        </div>

        <h2>What it generates</h2>
        <p>
          Each table gets its module set under <code>src/db/&lt;table&gt;/</code>{' '}
          — <code>models</code>, <code>read</code>, <code>write</code>,{' '}
          <code>filter</code>, <code>relationships</code> — plus optional
          API features (<code>src/features/**</code>) and{' '}
          <code>utoipa</code> OpenAPI schemas. The modules are
          model-shaped since 0.3.0: the data layer rides the facade's
          <code> Model</code> chains instead of raw sqlx functions.
        </p>
        <table className="w-full">
          <thead>
            <tr className="border-b">
              <th className="py-2 text-left">Command</th>
              <th className="py-2 text-left">What it does</th>
            </tr>
          </thead>
          <tbody>
            <tr className="border-b"><td className="py-2 font-mono text-sm">rok sqlc g</td><td className="py-2 text-sm">generate — reads rok-data(-sqlc).toml, or --schema schema.sql</td></tr>
            <tr className="border-b"><td className="py-2 font-mono text-sm">rok sqlc i</td><td className="py-2 text-sm">init — scaffold a project layout (actix or axum)</td></tr>
            <tr className="border-b"><td className="py-2 font-mono text-sm">rok sqlc check</td><td className="py-2 text-sm">freshness check — exit 1 when the output is out of date</td></tr>
            <tr className="border-b"><td className="py-2 font-mono text-sm">rok sqlc a / check-arch</td><td className="py-2 text-sm">architecture gate: the generated layout's rules (engine ADR 0016)</td></tr>
            <tr className="border-b"><td className="py-2 font-mono text-sm">rok sqlc query add NAME SQL</td><td className="py-2 text-sm">annotate custom queries into the query registry</td></tr>
          </tbody>
        </table>
        <p>
          The engine also powers the front door: <code>rok gen</code> and{' '}
          <code>rok init</code> delegate their heavy lifting here
          (<code>rok_data_sqlc::init</code> and the engine's generator),
          and the staged-pair diffing comes from the codegen crate.
        </p>

        <h2>Two API backends, one body</h2>
        <p>
          <code>[api] framework</code> (or{' '}
          <code>rok gen -w/--api-framework</code>) picks{' '}
          <code>actix_web</code> (the default) or <code>axum</code>; the
          per-framework emission sits in{' '}
          <code>generator/api/{'{'}actix,axum{'}'}.rs</code> while bodies,
          the service, listing and the OpenAPI document are shared.{' '}
          <code>check-arch</code> rule 1 names the configured framework.
        </p>

        <h2>Templates</h2>
        <p>
          <code>rok init --template actix</code> /{' '}
          <code>--template axum</code> copy complete starter applications —
          their own README, <code>AGENTS.md</code>, migrations and tests —
          pinned to the template lints (<code>deny(missing_docs)</code>,
          pedantic, no <code>unwrap</code>/<code>expect</code>/<code>panic</code>).
        </p>

        <h2>Conventions</h2>
        <p>
          The unified <code>[conventions]</code> key set (preset{' '}
          <code>rok</code> default; an empty string disables a key) feeds
          both the ORM derive generation and the engine's conventions
          machinery — one key set, two consumers (workspace ADR 0015).
        </p>

        <h2>Engine house</h2>
        <p>
          Its ADR series lives in{' '}
          <code>crates/rok-data-sqlc/docs/adr/</code> (0026 newest; ADRs
          0019–0022 imported from rok-sqlc). The expected-snapshot files
          refresh only with <code>ROK_DATA_SQLC_UPDATE_EXPECTED=1</code>.
        </p>

        <h2>Links</h2>
        <ul>
          <li><Link to="/crates/rok-data-core">rok-data-core — the runtime the generated code calls</Link></li>
          <li><Link to="/crates/rok-cli">the rok binary — the sqlc subtree forwards here</Link></li>
          <li><a href="https://github.com/ateeq1999/rok-data/tree/main/examples/api" target="_blank" rel="noreferrer">examples/api — the vertical slice</a></li>
        </ul>
      </article>
    </div>
  )
}
