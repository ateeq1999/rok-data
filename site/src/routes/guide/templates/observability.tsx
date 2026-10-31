import { Link, createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/guide/templates/observability')({
  component: TemplatesObservabilityGuide,
})

function TemplatesObservabilityGuide() {
  return (
    <div className="mx-auto max-w-4xl px-4 py-8">
      <article className="prose max-w-none">
        <h1>Observability</h1>
        <p className="lead">
          What the API tells you about itself: log lines, a span for every
          request and job, trace export over OpenTelemetry, and metrics in
          Prometheus's format. Each is in the template and inert until
          configured, so a new project logs to the terminal and exports
          nothing.
        </p>

        <h2>Logs</h2>
        <p>
          <code>src/shared/telemetry.rs</code> starts tracing before anything
          else, from <code>RUST_LOG</code> (the filter: <code>info</code>,
          <code>debug,sqlx=warn</code>) and <code>LOG_FORMAT</code>:
          <code>pretty</code>, one readable line per event (the default), or
          <code>json</code>, one object per line, for a log collector (the
          <code>Dockerfile</code> sets it). The server logs its settings at
          start-up with every secret left out (<code>JWT_SECRET=&lt;48
          bytes&gt;</code>, a URL's password <code>&lt;redacted&gt;</code>), so a wrong
          setting shows in the first lines of a deploy.
        </p>

        <h2>The request span</h2>
        <p>
          Every request runs in a span carrying its method, its route (the
          matched pattern, <code>{"/api/v1/users/{id}"}</code>, so a backend groups
          by it), its request id (answered as <code>x-request-id</code>, so a
          caller can quote it), and, once <code>shared/auth.rs</code> knows
          it, the caller's <code>user_id</code>. Every event logged while
          serving the request carries them. A job runs in a span of its own
          (<code>job</code>: kind, id, attempt), continuing the trace of the
          request that enqueued it.
        </p>

        <h2>Traces</h2>
        <p>
          Set <code>OTEL_EXPORTER_OTLP_ENDPOINT</code> (such as
          <code>http://otel-collector:4318</code>) and every span is exported
          over OTLP/HTTP; unset, nothing is exported or connected to. A request
          carrying a W3C <code>traceparent</code> continues the caller's trace.
          The exporter reads the standard variables itself:
        </p>
        <table>
          <thead>
            <tr><th>Variable</th><th></th></tr>
          </thead>
          <tbody>
            <tr><td><code>OTEL_EXPORTER_OTLP_ENDPOINT</code></td><td>the collector's base URL; set, spans are exported</td></tr>
            <tr><td><code>OTEL_EXPORTER_OTLP_HEADERS</code></td><td>headers to send, such as a vendor's API key (<code>x-honeycomb-team=…</code>)</td></tr>
            <tr><td><code>OTEL_SERVICE_NAME</code></td><td>the service's name; by default the crate's</td></tr>
            <tr><td><code>OTEL_TRACES_SAMPLER</code>, <code>OTEL_TRACES_SAMPLER_ARG</code></td><td>sampling, such as <code>parentbased_traceidratio</code> and <code>0.1</code></td></tr>
            <tr><td><code>OTEL_RESOURCE_ATTRIBUTES</code></td><td>more attributes, such as <code>deployment.environment=prod</code></td></tr>
          </tbody>
        </table>
        <p>
          Spans are batched and sent from a thread of their own; those still
          buffered are sent when the server stops. Any backend that takes OTLP
          works (Jaeger, Grafana Tempo, Honeycomb, Datadog, an OpenTelemetry
          Collector).
        </p>

        <h2>Metrics</h2>
        <p>
          Set <code>METRICS_ADDR</code> (<code>0.0.0.0:9090</code> in a
          container) and <code>/metrics</code> is served there, a listener of
          its own, so the API's port never shows it; keep that port private and
          let Prometheus scrape it. What is measured
          (<code>src/shared/metrics.rs</code>):
        </p>
        <table>
          <thead>
            <tr><th>Metric</th><th>Kind</th><th>Labels</th></tr>
          </thead>
          <tbody>
            <tr><td><code>http_requests_total</code></td><td>counter</td><td><code>method</code>, <code>route</code> (<code>unmatched</code> for a path no route matched), <code>status</code></td></tr>
            <tr><td><code>http_request_duration_seconds</code></td><td>histogram</td><td><code>method</code>, <code>route</code></td></tr>
            <tr><td><code>db_pool_connections</code></td><td>gauge</td><td><code>state</code>: <code>idle</code>, <code>in_use</code></td></tr>
            <tr><td><code>db_pool_max_connections</code></td><td>gauge</td><td></td></tr>
            <tr><td><code>jobs_total</code></td><td>counter</td><td><code>kind</code>, <code>outcome</code>: <code>done</code>, <code>failed</code></td></tr>
            <tr><td><code>job_duration_seconds</code></td><td>histogram</td><td><code>kind</code></td></tr>
            <tr><td><code>jobs_queued</code></td><td>gauge</td><td><code>state</code>: <code>due</code>, <code>scheduled</code>, <code>dead</code></td></tr>
            <tr><td><code>auth_logins_total</code></td><td>counter</td><td><code>outcome</code>: <code>success</code>, <code>two_factor</code>, <code>bad_credentials</code>, <code>locked</code>, <code>forbidden</code></td></tr>
            <tr><td><code>auth_lockouts_total</code></td><td>counter</td><td></td></tr>
          </tbody>
        </table>
        <p>Alerts worth having from the start:</p>
        <ul>
          <li>the 5xx rate: <code>{"sum(rate(http_requests_total{status=~\"5..\"}[5m])) > 0.1"}</code></li>
          <li>latency: the p95 of <code>http_request_duration_seconds</code> above your budget</li>
          <li>dead jobs: <code>{"jobs_queued{state=\"dead\"} > 0"}</code></li>
          <li>a queue falling behind: <code>{"jobs_queued{state=\"due\"}"}</code> growing for 10 minutes</li>
          <li>the pool exhausted: <code>{"db_pool_connections{state=\"in_use\"}"}</code> at <code>db_pool_max_connections</code></li>
          <li>credential stuffing: <code>{"rate(auth_logins_total{outcome=\"bad_credentials\"}[5m])"}</code> far above its usual level, <code>auth_lockouts_total</code> rising</li>
        </ul>

        <h2>Locally: Jaeger, Prometheus, Grafana</h2>
        <pre><code>{`$ OTEL_EXPORTER_OTLP_ENDPOINT=http://jaeger:4318 docker compose --profile observability up --build`}</code></pre>
        <p>
          Jaeger at <code>http://localhost:16686</code> shows the traces,
          Prometheus at <code>http://localhost:9091</code> scrapes
          <code>/metrics</code>, and Grafana at
          <code>http://localhost:3000</code> opens the <strong>API</strong>
          dashboard (<code>deploy/compose/grafana/dashboards/api.json</code>):
          requests and latency by route, server errors, the job queue, the
          pool and sign-ins.
        </p>

        <h2>Your own spans and metrics</h2>
        <p>
          <code>#[tracing::instrument]</code> on a service method, or
          <code>tracing::info_span!</code>, nests under the request's span and
          is exported with it. A metric is one line anywhere,
          <code>metrics::counter!("orders_placed_total").increment(1)</code>,
          exported with the others. Keep labels to a few known values: a label
          per user or per id is a series per user, and the bill follows. To
          send metrics over OTLP instead of serving <code>/metrics</code>, see
          the <Link to="/guide/templates/recipes/otel-metrics">recipe</Link>.
        </p>
      </article>
    </div>
  )
}