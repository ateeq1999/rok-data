import { Link, createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/guide/templates/security')({
  component: TemplatesSecurityGuide,
})

function TemplatesSecurityGuide() {
  return (
    <div className="mx-auto max-w-4xl px-4 py-8">
      <article className="prose max-w-none">
        <h1>Security</h1>
        <p className="lead">
          What the templates do for you, what you set, and what stays yours.
          The account flows' own rules (argon2id, single-use tokens, lockouts,
          two-factor, API keys) are in <Link to="/guide/templates/auth">Accounts
          and sign-in</Link>.
        </p>

        <h2>On every answer</h2>
        <p>
          <code>src/app/middleware.rs</code> applies
          <code>src/shared/security/headers.rs</code> (the same in both
          templates) to every answer, errors included:
        </p>
        <table>
          <thead>
            <tr><th>Header</th><th>Value</th><th>Why</th></tr>
          </thead>
          <tbody>
            <tr><td><code>X-Content-Type-Options</code></td><td><code>nosniff</code></td><td>a browser never guesses another type than the one sent</td></tr>
            <tr><td><code>X-Frame-Options</code></td><td><code>DENY</code></td><td>no page frames the API (clickjacking)</td></tr>
            <tr><td><code>Content-Security-Policy</code></td><td><code>frame-ancestors 'none'</code></td><td>the same, for current browsers</td></tr>
            <tr><td><code>Referrer-Policy</code></td><td><code>no-referrer</code></td><td>no URL leaks to the next site</td></tr>
            <tr><td><code>Strict-Transport-Security</code></td><td><code>max-age=31536000; includeSubDomains</code></td><td>only when <code>APP_URL</code> is <code>https://</code></td></tr>
          </tbody>
        </table>
        <p>A header a handler sets itself is kept. A handler that panics is
        answered <code>500 internal_error</code>, the panic logged, instead of
        the connection being dropped; the message never reaches the caller.</p>

        <h2>CORS</h2>
        <p>
          <code>CORS_ALLOWED_ORIGINS</code> names the front ends that call the
          API from another origin:
          <code>https://app.example.com,http://localhost:5173</code>, or
          <code>*</code>. Unset, no other origin is allowed. A request from an
          origin not listed is not refused; it gets no
          <code>Access-Control-Allow-Origin</code>, and the browser keeps the
          answer from the page. The API's credential is a bearer token, not a
          cookie, so there is no CSRF to defend against; keep it that way, or
          add CSRF protection with the cookie.
        </p>

        <h2>Rate limits</h2>
        <p>Two per-IP limits, counted in the process:</p>
        <ul>
          <li><code>AUTH_RATE_LIMIT</code> (20 a minute) on <code>/auth</code>:
            sign-in, sign-up, the password and email flows, two-factor.</li>
          <li><code>GLOBAL_RATE_LIMIT</code> (off by default) on every route
            but health, so a probe is never refused.</li>
        </ul>
        <p>Past one, the answer is <code>429 too_many_requests</code> with
        <code>Retry-After</code>. Behind a load balancer or a proxy you run,
        set <code>TRUST_PROXY=true</code>: the client IP is then the last
        <code>X-Forwarded-For</code> address, the one your proxy appended.
        Leave it off otherwise, since a client sets that header to anything.
        With several replicas each counts its own; for one count across them,
        see the <Link to="/guide/templates/recipes/redis-rate-limit">Redis
        recipe</Link>.</p>

        <h2>Secrets</h2>
        <ul>
          <li>
            <strong><code>JWT_SECRET</code></strong> signs the session tokens
            (HS256, at least 32 bytes). Rotate it without signing anyone out:
            set the new value as <code>JWT_SECRET</code> and the old one as
            <code>JWT_SECRET_PREVIOUS</code>, wait one <code>JWT_TTL</code>
            (every token the old one signed has expired), then remove
            <code>JWT_SECRET_PREVIOUS</code>. The download links the API signs
            itself (for a store that can't sign its own) use a key derived from
            it, so those handed out before the switch stop working; they last
            minutes.
          </li>
          <li>
            <strong><code>TOTP_ENCRYPTION_KEY</code></strong> seals the
            two-factor secrets at rest (32 bytes, base64). It can't be rotated
            by setting a new one: the stored secrets are sealed with the old
            key. Keep it as safe as the database backups.
          </li>
          <li>
            <strong>Where they live</strong>: in production, in a secret store
            injected as environment variables (<code>deploy/terraform/</code>'s
            Secrets Manager); never in the image, a task definition or the
            repository. <code>Config</code> has no <code>Debug</code>, and its
            start-up summary prints a secret's length only.
          </li>
        </ul>

        <h2>Dependencies</h2>
        <p>
          <code>deny.toml</code> is <code>cargo deny check</code>'s policy,
          and CI runs it: no crate with a known advisory, licenses from an
          allow list, crates from crates.io only. The advisories it ignores are
          listed there, each with why it can't reach the project (HS256-only
          tokens, a build-time <code>zip</code>, the configured S3 store,
          actix's TLS-only HTTP/2). When <code>cargo update</code> clears one,
          drop its entry; review the list on every dependency update. The image
          runs on distroless, as a non-root user, with the one binary in it.
        </p>

        <h2>What stays yours</h2>
        <ul>
          <li><strong>TLS</strong>: terminate it at the load balancer
            (<code>deploy/terraform</code>'s <code>certificate_arn</code>); the
            API serves plain HTTP behind it.</li>
          <li><strong>Authorization</strong> beyond the two roles: who may read
            and change what is each feature's rule, in its <code>service.rs</code>.</li>
          <li><strong>Input limits</strong> beyond the 2 MiB body limit and the
            upload limits: page sizes are capped; anything else that is
            expensive per request (a search over everything, an export) needs
            its own limit.</li>
          <li><strong>Audit</strong>: admins' actions on accounts and jobs are
            logged (<code>audit_log</code>); log your own sensitive actions
            through the same port.</li>
        </ul>
      </article>
    </div>
  )
}