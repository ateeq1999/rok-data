import { Link, createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/guide/templates/')({
  component: TemplatesGuideIndex,
})

function TemplatesGuideIndex() {
  return (
    <div className="mx-auto max-w-4xl px-4 py-8">
      <article className="prose dark:prose-invert max-w-none">
        <h1>The templates' guide</h1>
        <p className="lead">
          <code>rok sqlc init --template axum</code> (or <code>--template actix</code>)
          makes an API that is ready for production: accounts, uploads,
          background jobs, logs, traces, metrics, a security baseline, a Docker
          image, a local stack, CI and an AWS deployment. This guide is about
          what the templates ship and how you grow them; how rok-data-sqlc
          generates the API itself is <Link to="/crates/rok-data-sqlc">the
          engine's guide</Link>.
        </p>

        <p>
          Everything here works the same in both templates: the parts with no
          framework in them are the same file, byte for byte, in both
          (<code>tests/template_drift.rs</code> checks it). Each <strong>Capability</strong>
          is compiled in and inert until its environment variable turns it on.
        </p>

        <h2>What the API does</h2>
        <ul>
          <li>
            <Link to="/guide/templates/auth"><strong>Accounts and sign-in</strong></Link>:
            sign-up, sign-in, the password and email flows, two-factor sign-in,
            API keys, sessions, mail.
          </li>
          <li>
            <Link to="/guide/templates/files"><strong>Files and storage</strong></Link>:
            uploads, direct uploads, profile photos, the storage port, S3.
          </li>
          <li>
            <Link to="/guide/templates/jobs"><strong>Jobs and schedules</strong></Link>:
            background work on PostgreSQL, scheduled jobs, <code>APP_ROLE</code>,
            failed jobs, testing jobs.
          </li>
        </ul>

        <h2>Running it in production</h2>
        <ul>
          <li>
            <Link to="/guide/templates/observability"><strong>Observability</strong></Link>:
            logs, the request span, trace export (OpenTelemetry), metrics
            (Prometheus), the local Grafana.
          </li>
          <li>
            <Link to="/guide/templates/security"><strong>Security</strong></Link>:
            the headers, CORS, rate limits, panics, rotating the secrets, the
            dependency policy.
          </li>
          <li>
            <Link to="/guide/templates/deploying-aws"><strong>Deploying to AWS</strong></Link>:
            the Terraform, a release, and what the Floci test proves and what it
            does not.
          </li>
        </ul>

        <h2>Recipes</h2>
        <p>
          Integrations that pick a vendor or an opt-in dependency are not in the
          templates; these pages add them to your project:
        </p>
        <ul>
          <li>
            <a href="https://github.com/ateeq1999/rok-data/blob/main/crates/rok-templates/docs/guide/recipes/sentry.md" target="_blank" rel="noreferrer"><strong>Errors to Sentry</strong></a>
          </li>
          <li>
            <a href="https://github.com/ateeq1999/rok-data/blob/main/crates/rok-templates/docs/guide/recipes/otel-metrics.md" target="_blank" rel="noreferrer"><strong>Metrics over OTLP</strong></a>
            instead of Prometheus
          </li>
          <li>
            <a href="https://github.com/ateeq1999/rok-data/blob/main/crates/rok-templates/docs/guide/recipes/redis-rate-limit.md" target="_blank" rel="noreferrer"><strong>A shared rate limit on Redis</strong></a>
            for many replicas
          </li>
          <li>
            <a href="https://github.com/ateeq1999/rok-data/blob/main/crates/rok-templates/docs/guide/recipes/webhooks.md" target="_blank" rel="noreferrer"><strong>Signed outbound webhooks</strong></a>,
            delivered as jobs
          </li>
          <li>
            <a href="https://github.com/ateeq1999/rok-data/blob/main/crates/rok-templates/docs/guide/recipes/oidc.md" target="_blank" rel="noreferrer"><strong>Sign-in with OIDC</strong></a>
            (an identity provider)
          </li>
        </ul>
      </article>
    </div>
  )
}