import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/agents')({
  component: AgentsPage,
})

function AgentsPage() {
  return (
    <div className="mx-auto max-w-4xl px-4 py-8">
      <article className="prose dark:prose-invert max-w-none">
        <h1>rok-data for coding agents</h1>
        <p className="lead">
          One truth, one command per step, one JSON document when a program
          asks for it. Paste this page into a coding agent and it can
          scaffold, generate and verify a rok-data project unattended.
        </p>

        <h2>The mental model</h2>
        <p>
          The <strong>schema directory</strong> is the only source of
          truth. <code>rok gen</code> turns it into a Rust data layer and
          API features; <code>rok gen --migration NAME</code> stages the
          difference since the last staged pair;{' '}
          <code>rok gen check</code> exits <strong>1</strong> when the
          generated code is stale — the freshness oracle.
        </p>

        <h2>JSON contracts</h2>
        <pre><code className="language-sh">$ rok migrate status --json</code></pre>
        <pre><code>{`{
  "migrations": [
    { "version": 1, "name": "create_notes", "state": "applied",
      "checksum": "9f2a31b8c04d5177", "drift": false }
  ]
}`}</code></pre>
        <p>
          <code>state</code> ∈ <code>applied | pending | edited | unknown</code>;
          drift <code>true</code> is fatal to apply without a deliberate
          decision. <code>rok db check --json</code> reports{' '}
          <code>{'{'}"connected": true, "server_version": "…"{'}'}</code>.
        </p>

        <h2>Exit codes</h2>
        <table className="w-full">
          <thead>
            <tr className="border-b"><th className="py-2 text-left">Code</th><th className="py-2 text-left">Meaning</th></tr>
          </thead>
          <tbody>
            <tr className="border-b"><td className="py-2 font-mono text-sm">0</td><td className="py-2 text-sm">success, --help, --version</td></tr>
            <tr className="border-b"><td className="py-2 font-mono text-sm">1</td><td className="py-2 text-sm">runtime failure (bad URL, drift, failed check); usage errors inside `rok sqlc …`</td></tr>
            <tr className="border-b"><td className="py-2 font-mono text-sm">2</td><td className="py-2 text-sm">usage errors at the root (clap)</td></tr>
          </tbody>
        </table>

        <h2>The unattended loop</h2>
        <pre><code>{`# 1. Edit the schema (the only truth).
$EDITOR schema/0002_add_posts.sql

# 2. Ask for the plan first; write nothing.
rok gen --dry-run

# 3. Stage the delta as a pair + generate the code.
rok gen --migration add_posts

# 4. Verify freshness (CI mode).
rok gen check                      # exit 0 = fresh

# 5. Apply and confirm.
export DATABASE_URL=postgres://…
rok migrate up && rok migrate status --json`}</code></pre>

        <h2>Rules the loop relies on</h2>
        <ul>
          <li>
            <code>src/db/**</code> and <code>src/features/**</code> carry{' '}
            <code>@generated</code> markers — change the schema, not the
            generated files.
          </li>
          <li>
            <code>rok init</code> writes every file once, never rewritten.
          </li>
          <li>
            Scaffolded projects ship their own <code>AGENTS.md</code>,
            pinned to <code>rok sqlc check-arch</code>'s layout rules.
          </li>
          <li>
            Destructive steps refuse without{' '}
            <code>--allow-destructive</code>; <code>--rename</code>{' '}
            preserves identity instead of drop+create.
          </li>
        </ul>

        <h2>In-repo agent skills</h2>
        <p>
          The repository ships five OpenCode/Claude/AGENTS-spec skills
          under <code>.agents/skills/rok-data*/</code>:
        </p>
        <table className="w-full">
          <thead>
            <tr className="border-b"><th className="py-2 text-left">Skill</th><th className="py-2 text-left">Fires when</th></tr>
          </thead>
          <tbody>
            <tr className="border-b"><td className="py-2 font-mono text-sm">rok-data</td><td className="py-2 text-sm">any work in the workspace — the router</td></tr>
            <tr className="border-b"><td className="py-2 font-mono text-sm">rok-data-init</td><td className="py-2 text-sm">scaffold + generate a service end to end</td></tr>
            <tr className="border-b"><td className="py-2 font-mono text-sm">rok-data-models</td><td className="py-2 text-sm">authoring model code (derive, chains, verbs)</td></tr>
            <tr className="border-b"><td className="py-2 font-mono text-sm">rok-data-migrations</td><td className="py-2 text-sm">migration design, staging, drift, JSON contract</td></tr>
            <tr className="border-b"><td className="py-2 font-mono text-sm">rok-data-sqlc</td><td className="py-2 text-sm">engine internals inside crates/rok-data-sqlc</td></tr>
          </tbody>
        </table>

        <h2>Machine index</h2>
        <p>
          The full markdown docs live in the repository's{' '}
          <a href="https://github.com/ateeq1999/rok-data/tree/main/docs" target="_blank" rel="noreferrer">docs/</a>,
          every page self-contained for pasting; the index for machines is{' '}
          <a href="https://raw.githubusercontent.com/ateeq1999/rok-data/main/docs/llms.txt" target="_blank" rel="noreferrer">docs/llms.txt</a>.
        </p>
      </article>
    </div>
  )
}
