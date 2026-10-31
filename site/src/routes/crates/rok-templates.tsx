import { Link, createFileRoute } from '@tanstack/react-router'
import { Badge } from '../../components/ui/badge'

export const Route = createFileRoute('/crates/rok-templates')({
  component: RokTemplatesDocs,
})

function RokTemplatesDocs() {
  return (
    <div className="mx-auto max-w-4xl px-4 py-8">
      <article className="prose max-w-none">
        <h1>rok-templates</h1>
        <p className="text-lg text-muted-foreground">
          The project templates <code>rok sqlc init --template</code> copies into
          a new project: complete actix-web (<code>templates/actix-api/</code>) and
          axum (<code>templates/axum-api/</code>) APIs, with their schema, migrations,
          hand-written account flows and generated API features.
        </p>

        <div className="not-prose my-4 flex items-center gap-4">
          <Badge
            variant="outline"
            render={<a href="https://crates.io/crates/rok-templates" target="_blank" rel="noreferrer" />}
          >
            crates.io
          </Badge>
          <a
            href="https://github.com/ateeq1999/rok-data/tree/main/crates/rok-templates"
            target="_blank"
            rel="noreferrer"
            className="text-sm text-muted-foreground no-underline hover:underline"
          >
            source: crates/rok-templates
          </a>
        </div>

        <h2>Usage</h2>
        <p>You don't depend on this crate yourself. Install the CLI and start a project from a template:</p>
        <pre><code>{`$ cargo install rok-data-cli
$ rok sqlc init --template axum
# or
$ rok sqlc init --template actix`}</code></pre>

        <h2>What the templates ship</h2>
        <p>
          Each template is a complete API with:
        </p>
        <ul>
          <li><strong>Accounts and sign-in</strong> — sign-up, email verification, sign-in, password reset/change, email change, invitations, two-factor sign-in (TOTP), API keys</li>
          <li><strong>Files and storage</strong> — uploads, direct uploads, profile photos, the storage port, S3 and S3-compatible stores</li>
          <li><strong>Jobs and schedules</strong> — background work on PostgreSQL, scheduled jobs, <code>APP_ROLE</code>, failed jobs, testing jobs</li>
          <li><strong>Observability</strong> — logs, the request span, trace export (OpenTelemetry), metrics (Prometheus), local Grafana</li>
          <li><strong>Security</strong> — security headers, CORS, rate limits, secrets, dependency policy</li>
          <li><strong>Deployment</strong> — Dockerfile, compose stack, CI, and an AWS reference deployment in Terraform</li>
        </ul>

        <p>
          Everything works the same in both templates: the parts with no framework
          in them are the same file, byte for byte, in both
          (<code>tests/template_drift.rs</code> checks it). Each capability is compiled
          in and inert until its environment variable turns it on.
        </p>

        <h2>The templates' guide</h2>
        <p>
          The guide explains what the templates ship and how you grow them:
        </p>
        <ul>
          <li><Link to="/guide/templates">Overview</Link></li>
          <li><Link to="/guide/templates/auth">Accounts and sign-in</Link></li>
          <li><Link to="/guide/templates/files">Files and storage</Link></li>
          <li><Link to="/guide/templates/jobs">Jobs and schedules</Link></li>
          <li><Link to="/guide/templates/observability">Observability</Link></li>
          <li><Link to="/guide/templates/security">Security</Link></li>
          <li><Link to="/guide/templates/deploying-aws">Deploying to AWS</Link></li>
          <li>Recipes:
            <ul>
              <li><a href="https://github.com/ateeq1999/rok-data/blob/main/crates/rok-templates/docs/guide/recipes/sentry.md" target="_blank" rel="noreferrer">Errors to Sentry</a></li>
              <li><a href="https://github.com/ateeq1999/rok-data/blob/main/crates/rok-templates/docs/guide/recipes/otel-metrics.md" target="_blank" rel="noreferrer">Metrics over OTLP</a></li>
              <li><a href="https://github.com/ateeq1999/rok-data/blob/main/crates/rok-templates/docs/guide/recipes/redis-rate-limit.md" target="_blank" rel="noreferrer">A shared rate limit on Redis</a></li>
              <li><a href="https://github.com/ateeq1999/rok-data/blob/main/crates/rok-templates/docs/guide/recipes/webhooks.md" target="_blank" rel="noreferrer">Signed outbound webhooks</a></li>
              <li><a href="https://github.com/ateeq1999/rok-data/blob/main/crates/rok-templates/docs/guide/recipes/oidc.md" target="_blank" rel="noreferrer">Sign-in with OIDC</a></li>
            </ul>
          </li>
        </ul>

        <h2>How it works</h2>
        <p>
          The crate is data only: <code>build.rs</code> embeds <code>templates/</code>,
          and <code>EMBEDDED</code> lists it. The engine, <code>rok-data-sqlc</code>,
          decides what a project gets (workspace ADR 0022 in the repository's
          <code>docs/decisions/</code>).
        </p>

        <h2>Links</h2>
        <ul>
          <li><Link to="/guide/templates">The templates' guide</Link></li>
          <li><Link to="/crates/rok-data-sqlc">rok-data-sqlc — the engine that copies the templates</Link></li>
          <li><a href="https://github.com/ateeq1999/rok-data/tree/main/crates/rok-templates/templates" target="_blank" rel="noreferrer">Template source (actix-api, axum-api)</a></li>
        </ul>
      </article>
    </div>
  )
}