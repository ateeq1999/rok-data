import { Link, createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/crates/rok-cli')({
  component: RokCliDocs,
})

function RokCliDocs() {
  return (
    <div className="mx-auto max-w-4xl px-4 py-8">
      <article className="prose dark:prose-invert max-w-none">
        <h1>rok (the CLI)</h1>
        <p className="text-lg text-muted">
          The workspace's single binary. Published as <strong>rok-data-cli</strong>,
          the binary is <strong>rok</strong>.
        </p>

        <div className="my-4 flex items-center gap-4">
          <a
            href="https://crates.io/crates/rok-data-cli"
            target="_blank"
            rel="noreferrer"
            className="no-underline rounded-full bg-[var(--chip-bg)] border border-[var(--chip-line)] px-3 py-1.5 text-sm font-semibold"
          >
            crates.io: rok-data-cli
          </a>
          <a
            href="https://github.com/ateeq1999/rok-data/tree/main/crates/rok-cli"
            target="_blank"
            rel="noreferrer"
            className="text-sm text-muted-foreground no-underline hover:underline"
          >
            source: crates/rok-cli
          </a>
        </div>

        <h2>Installation</h2>
        <pre><code>cargo install rok-data-cli</code></pre>

        <h2>Command surface</h2>
        <table className="w-full">
          <thead>
            <tr className="border-b">
              <th className="py-2 text-left">Command</th>
              <th className="py-2 text-left">What it does</th>
            </tr>
          </thead>
          <tbody>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">rok init [NAME]</td>
              <td className="py-2 text-sm">scaffold a project (skeleton or --template actix/axum); schema seeded</td>
            </tr>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">rok gen [OPTIONS] [check]</td>
              <td className="py-2 text-sm">the data layer and API features from the schema dir; --migration stages a pair</td>
            </tr>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">rok migrate &lt;COMMAND&gt;</td>
              <td className="py-2 text-sm">create | status | up | down (alias rollback) — journal-checked pairs</td>
            </tr>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">rok db &lt;COMMAND&gt;</td>
              <td className="py-2 text-sm">check | inspect — connectivity and structure</td>
            </tr>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">rok schema &lt;COMMAND&gt;</td>
              <td className="py-2 text-sm">dump | diff — the live schema as DDL; journal-vs-files diff</td>
            </tr>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">rok sqlc &lt;COMMAND&gt;</td>
              <td className="py-2 text-sm">legacy: the engine's grammar, forwarded verbatim (g / i / check / a / query …)</td>
            </tr>
            <tr className="border-b">
              <td className="py-2 font-mono text-sm">rok generate [OPTIONS] [check]</td>
              <td className="py-2 text-sm">legacy: whole-crate generation from .sql files</td>
            </tr>
          </tbody>
        </table>

        <h2>Contracts a machine can rely on</h2>
        <ul>
          <li>
            <code>--database-url &lt;URL&gt;</code> on every
            database-touching command; falls back to{' '}
            <code>DATABASE_URL</code>. URLs print masked.
          </li>
          <li>
            <code>--json</code> (migrate/db, and check modes): one JSON
            document on stdout, nothing else there; progress and prose on
            stderr. Shapes:{' '}
            <code>{'{'}"migrations": [{'{'}version, name, state: "applied|pending|edited|unknown", checksum, drift{'}'}]{'}'}</code>{' '}
            and{' '}
            <code>{'{'}"connected": true, "server_version": "…"{'}'}</code>.
          </li>
          <li>
            Exit codes: <code>--help</code>/<code>--version</code> = 0;
            root usage errors = 2 (clap); runtime failures = 1 — *usage*
            errors inside <code>rok sqlc …</code> exit 1, the engine's
            historic behavior.
          </li>
          <li>
            <code>--dry-run</code> plans and writes nothing;{' '}
            <code>--strict</code> turns warnings into failures;{' '}
            <strong>flags beat the settings file</strong>.
          </li>
        </ul>

        <h2>The two legacy subtrees</h2>
        <p>
          <code>rok generate</code> and <code>rok sqlc …</code> keep their
          flags, output and exit codes; they are superseded by the front
          door (<code>rok init</code> scaffolds, <code>rok gen</code>{' '}
          generates) and their removal is deferred to 1.0 (engine ADR
          0012).
        </p>

        <h2>Example: the unattended loop</h2>
        <pre><code>{`$ export DATABASE_URL=postgres://postgres:postgres@localhost:5432/my_app
$ rok gen --dry-run              # plan, write nothing
$ rok gen --migration add_posts  # stage the delta + generate
$ rok gen check                  # exit 0 = generated code is fresh
$ rok migrate up && rok migrate status --json`}</code></pre>

        <h2>Links</h2>
        <ul>
          <li><Link to="/agents">For coding agents — contracts in full</Link></li>
          <li><a href="https://github.com/ateeq1999/rok-data/blob/main/docs/cli-reference.md" target="_blank" rel="noreferrer">docs/cli-reference.md — the contract page</a></li>
          <li><Link to="/crates/rok-data">rok-data — the runtime this CLI drives</Link></li>
        </ul>
      </article>
    </div>
  )
}
