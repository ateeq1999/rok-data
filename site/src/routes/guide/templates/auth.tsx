import { Link, createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/guide/templates/auth')({
  component: TemplatesAuthGuide,
})

function TemplatesAuthGuide() {
  return (
    <div className="mx-auto max-w-4xl px-4 py-8">
      <article className="prose max-w-none">
        <h1>Accounts and sign-in (the templates)</h1>
        <p>
          <code>rok sqlc init --template actix</code> (or <code>--template axum</code>) makes an
          API whose users sign up, verify their email, sign in (with a second
          factor if they turn it on), reset or change their password, change
          their email, make API keys for their scripts, and are invited by an
          admin. These flows are hand-written features in the template, not
          generated code: <code>src/features/auth/</code>,
          <code>two_factor/</code>, <code>api_keys/</code> and <code>account/</code>
          are yours, like every <code>service.rs</code>. The decisions behind them
          are in engine ADR 0020 and engine ADR 0022. Uploads and profile photos
          have <Link to="/guide/templates/files">their own page</Link>. A plain
          <code>rok sqlc init</code> has none of this, only the <code>AuthUser</code>
          and <code>AdminUser</code> checks of a token something else issues.
        </p>

        <p>
          The two templates hold the same flows: the services, DTOs, queries,
          migrations, <code>shared/security/</code>, <code>shared/mail/</code>,
          <code>shared/storage/</code>, <code>shared/thumbnail.rs</code> and the
          rate limit's <code>limiter.rs</code> are byte-identical (a repository
          test, <code>template_drift</code>, checks it), and only the handlers,
          the routes, the extractors and <code>app.rs</code> are written for the
          framework. Where this page shows actix-web wiring, the axum template
          does the same in axum's terms.
        </p>

        <h2>Where the code is</h2>
        <table>
          <thead>
            <tr>
              <th>Path</th>
              <th>What it holds</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td><code>src/features/auth/</code></td>
              <td>sign-up, sign-in, email verification, the password flows, email change, invitations</td>
            </tr>
            <tr>
              <td><code>src/features/two_factor/</code></td>
              <td><code>/auth/2fa/setup</code>, <code>enable</code>, <code>disable</code> and <code>verify</code></td>
            </tr>
            <tr>
              <td><code>src/features/api_keys/</code></td>
              <td><code>GET</code> and <code>POST /api-keys</code>, <code>{"DELETE /api-keys/{id}"}</code></td>
            </tr>
            <tr>
              <td><code>src/features/account/</code></td>
              <td><code>GET /auth/me</code>, <code>GET</code> and <code>PATCH /users/me</code>, the profile photo</td>
            </tr>
            <tr>
              <td><code>src/shared/auth.rs</code></td>
              <td>the <code>AuthUser</code> and <code>AdminUser</code> extractors</td>
            </tr>
            <tr>
              <td><code>src/shared/security/</code></td>
              <td>argon2id passwords, single-use tokens, JWT, breach check, TOTP, API keys</td>
            </tr>
            <tr>
              <td><code>src/shared/mail/</code></td>
              <td>the <code>Mailer</code> port, its SMTP and log transports, the messages, the outbox and its sender</td>
            </tr>
            <tr>
              <td><code>src/shared/rate_limit/</code></td>
              <td>the per-IP limit on <code>/auth</code></td>
            </tr>
            <tr>
              <td><code>queries/auth.sql</code></td>
              <td>the <code>auth</code> feature's SQL</td>
            </tr>
            <tr>
              <td><code>schema/0002_auth.sql</code>, <code>schema/0005_jobs.sql</code></td>
              <td><code>auth_tokens</code>, and <code>jobs</code>, the queue mail goes through</td>
            </tr>
            <tr>
              <td><code>schema/0003_two_factor.sql</code>, <code>0004_api_keys.sql</code></td>
              <td><code>totp_recovery_codes</code>, the <code>two_factor_pending</code> purpose; <code>api_keys</code> indexes</td>
            </tr>
          </tbody>
        </table>

        <p>
          <code>check-arch</code> applies in full: <code>auth</code> writes
          through its own query module, reads <code>users</code> through the
          data layer, and records audit entries through a port onto the
          <code>audit_log</code> service.
        </p>

        <h2>The flows</h2>
        <p>
          Every path is under <code>/api/v1</code>, and every <code>/auth</code>
          path is rate-limited. A body of the wrong shape (an email that isn't
          one, a password outside 8 to 128 characters) is a <code>422
          validation_failed</code>; a request a flow refuses is a <code>400</code>
          with its own code.
        </p>

        <table>
          <thead>
            <tr>
              <th>Endpoint</th>
              <th>Caller</th>
              <th>Answers</th>
            </tr>
          </thead>
          <tbody>
            <tr><td><code>POST /auth/register</code></td><td>anyone</td><td><code>201</code> the user, <code>400 breached_password</code>, <code>409</code></td></tr>
            <tr><td><code>POST /auth/login</code></td><td>anyone</td><td><code>200</code> a token, or <code>{'{requires_totp, pending_token}'}</code>; <code>401</code>, <code>403</code></td></tr>
            <tr><td><code>POST /auth/2fa/setup</code></td><td>user</td><td><code>200</code> the secret, its URI and QR code; <code>400</code></td></tr>
            <tr><td><code>POST /auth/2fa/enable</code></td><td>user</td><td><code>200</code> ten recovery codes; <code>400</code>, <code>422</code></td></tr>
            <tr><td><code>POST /auth/2fa/disable</code></td><td>user</td><td><code>200</code>, <code>400 wrong_password</code></td></tr>
            <tr><td><code>POST /auth/2fa/verify</code></td><td>anyone</td><td><code>200</code> a token, <code>400 invalid_token</code>, <code>401</code>, <code>403</code></td></tr>
            <tr><td><code>GET /auth/me</code></td><td>user</td><td><code>200</code> the user</td></tr>
            <tr><td><code>POST /auth/email/verify</code></td><td>anyone</td><td><code>200</code>, <code>400 invalid_token</code></td></tr>
            <tr><td><code>POST /auth/email/verification/resend</code></td><td>user</td><td><code>202</code></td></tr>
            <tr><td><code>POST /auth/password/forgot</code></td><td>anyone</td><td><code>202</code></td></tr>
            <tr><td><code>POST /auth/password/reset</code></td><td>anyone</td><td><code>200</code>, <code>400 invalid_token</code> or <code>breached_password</code></td></tr>
            <tr><td><code>POST /auth/password/change</code></td><td>user</td><td><code>200</code>, <code>400 wrong_password</code> or <code>breached_password</code></td></tr>
            <tr><td><code>POST /auth/email/change</code></td><td>user</td><td><code>202</code>, <code>400 wrong_password</code></td></tr>
            <tr><td><code>POST /auth/email/change/confirm</code></td><td>anyone</td><td><code>200</code>, <code>400 invalid_token</code></td></tr>
            <tr><td><code>POST /auth/invitations</code></td><td>admin</td><td><code>201</code> the user, <code>409</code></td></tr>
            <tr><td><code>GET /users/me</code>, <code>PATCH /users/me</code></td><td>user</td><td><code>200</code> the user</td></tr>
            <tr><td><code>GET /api-keys</code>, <code>POST /api-keys</code></td><td>user (session only)</td><td><code>200</code> the active keys; <code>201</code> a new key once, <code>409</code>, <code>422</code></td></tr>
            <tr><td><code>{"DELETE /api-keys/{id}"}</code></td><td>user (session only)</td><td><code>204</code>, <code>404</code></td></tr>
          </tbody>
        </table>

        <h2>Two-factor sign-in</h2>
        <p>
          A signed-in user turns on a second factor from an authenticator app
          (TOTP, RFC 6238: 6 digits, 30-second steps, one step of drift either
          way).
        </p>
        <ol>
          <li>
            <strong><code>POST /auth/2fa/setup</code></strong> answers
            <code>{"{\"secret\", \"qr_code_data_uri\", \"provisioning_uri\"}"}</code>: a
            new secret in base32 (to type in), its <code>otpauth://</code> URI
            and that URI as an SVG QR code (a <code>data:</code> URI for an
            <code>&lt;img src&gt;</code>). Nothing is enforced yet; a new setup
            replaces an unconfirmed one. With two-factor already on it is a
            <code>400</code>.
          </li>
          <li>
            <strong><code>POST /auth/2fa/enable</code></strong>
            <code>{"{\"code\"}"}</code> checks a code from the app against that
            secret and turns two-factor on. It answers
            <code>{"{\"recovery_codes\"}"}</code>, ten single-use
            <code>XXXXX-XXXXX</code> codes, shown this once, and mails a notice.
            A wrong code, or no setup, is a <code>400</code>.
          </li>
          <li>
            From then on, <strong><code>POST /auth/login</code></strong> with
            the right password answers
            <code>{"{\"requires_totp\": true, \"pending_token\"}"}</code> instead of a
            session. The pending token lasts 5 minutes and is single-use.
          </li>
          <li>
            <strong><code>POST /auth/2fa/verify</code></strong>
            <code>{"{\"pending_token\", \"code\"}"}</code> trades it, with a code from
            the app or an unused recovery code, for the session login would
            have answered. A bad, expired or used pending token is a
            <code>400 invalid_token</code>; a wrong code is a <code>401</code>
            and counts toward the lockout like a wrong password (5 in a row
            lock the account for 15 minutes), and the pending token stays
            usable until it expires.
          </li>
          <li>
            <strong><code>POST /auth/2fa/disable</code></strong>
            <code>{"{\"current_password\"}"}</code> turns it off, forgets the secret
            and the recovery codes, mails a notice, and signs every session
            out, the caller's included (<code>400 wrong_password</code> for a
            wrong password).
          </li>
        </ol>

        <p>
          <strong>At rest.</strong> The secret is stored in
          <code>users.totp_secret</code> sealed with AES-256-GCM under
          <code>TOTP_ENCRYPTION_KEY</code>, with the account's id as associated
          data, so a sealed secret copied onto another row doesn't open. The
          recovery codes are stored as HMAC-SHA256 under a key derived from
          <code>TOTP_ENCRYPTION_KEY</code>, never as themselves, so the table
          alone can't be checked against guesses.
        </p>

        <h2>API keys</h2>
        <p>
          A user makes keys for scripts and integrations:
          <code>POST /api-keys</code> with <code>{"{\"name\", \"scope\"?, \"expires_in_days\"?}"}</code>
          answers <code>201</code> with the key,
          <code>rok_&lt;8 characters&gt;_&lt;32 characters&gt;</code>, in
          <code>key</code>. It is shown this once; only its first 12 characters
          (<code>key_prefix</code>, to recognise it) and its SHA-256 are
          stored.
        </p>
        <ul>
          <li><strong>Sending one.</strong> <code>Authorization: Bearer rok_…</code>
            or <code>X-API-Key: rok_…</code>. <code>Authorization</code> wins
            when both are sent.</li>
          <li><strong>What it reaches.</strong> Every route <code>AuthUser</code>
            guards, acting as its owner, with the owner's role, an admin's
            included. No key reaches <code>/api/v1/auth/*</code> or
            <code>/api/v1/api-keys</code> (<code>403 forbidden</code>).</li>
          <li><strong>Scope.</strong> <code>read</code> (the default) may only
            <code>GET</code>, <code>HEAD</code> and <code>OPTIONS</code>;
            <code>write</code> may do what its owner may.</li>
          <li><strong>Lifetime.</strong> <code>expires_in_days</code> is 1 to
            365, or absent for a key that doesn't expire. An expired, revoked
            or unknown key is a <code>401</code>.</li>
          <li><strong>Limits.</strong> At most 25 active keys per user: the
            26th is a <code>409 conflict</code> until one is revoked.</li>
          <li><strong>Recovery.</strong> A password reset revokes every key of
            the account; a password change keeps them.</li>
        </ul>

        <h2>Sessions</h2>
        <p>
          A session is an HS256 JWT signed with <code>JWT_SECRET</code>,
          carrying <code>sub</code> (the user's id), <code>role</code>,
          <code>tv</code> (the account's <code>token_version</code>) and
          <code>exp</code>. There is no refresh token: a client signs in again
          when it expires (<code>JWT_TTL</code>).
        </p>
        <p>
          <code>AuthUser</code> checks the account on every request: a deleted
          or disabled account, or a token whose <code>tv</code> isn't the
          account's current <code>token_version</code>, is a <code>401</code>.
          A reset, a password change or turning two-factor off bumps
          <code>token_version</code>, which signs every older token out.
        </p>

        <h2>Settings</h2>
        <table>
          <thead>
            <tr><th>Variable</th><th>Default</th><th>Setting</th></tr>
          </thead>
          <tbody>
            <tr><td><code>JWT_SECRET</code></td><td>required</td><td>the HS256 key, at least 32 bytes (<code>openssl rand -base64 48</code>)</td></tr>
            <tr><td><code>JWT_TTL</code></td><td><code>3600</code></td><td>seconds a session token lasts</td></tr>
            <tr><td><code>TOTP_ENCRYPTION_KEY</code></td><td>required</td><td>the AES-256 key two-factor secrets are sealed under (32 bytes in base64)</td></tr>
            <tr><td><code>REQUIRE_VERIFIED_EMAIL</code></td><td><code>true</code></td><td>sign-in answers <code>403</code> until the email is verified</td></tr>
            <tr><td><code>APP_URL</code></td><td><code>http://localhost:8080</code></td><td>the base of mailed links: your front end's URL</td></tr>
            <tr><td><code>MAIL_TRANSPORT</code></td><td><code>log</code></td><td><code>log</code> (each mail is logged) or <code>smtp</code></td></tr>
            <tr><td><code>SMTP_URL</code></td><td>required for <code>smtp</code></td><td>the relay, such as <code>smtps://user:pass@host:465</code></td></tr>
            <tr><td><code>MAIL_FROM</code></td><td><code>Actix API &lt;no-reply@localhost&gt;</code></td><td>the sender of every mail</td></tr>
            <tr><td><code>BREACH_CHECK</code></td><td><code>false</code></td><td>refuse passwords Have I Been Pwned knows</td></tr>
            <tr><td><code>TRUST_PROXY</code></td><td><code>false</code></td><td>take the client IP from <code>X-Forwarded-For</code></td></tr>
            <tr><td><code>AUTH_RATE_LIMIT</code></td><td><code>20</code></td><td>requests a minute per IP on <code>/auth</code>; <code>0</code> turns it off</td></tr>
          </tbody>
        </table>

        <h2>Sending real mail</h2>
        <p>Set <code>MAIL_TRANSPORT=smtp</code>, <code>SMTP_URL</code> and <code>MAIL_FROM</code>:</p>
        <pre><code>{`MAIL_TRANSPORT=smtp
SMTP_URL=smtps://apikey:SECRET@smtp.example.com:465      # TLS from the start
# SMTP_URL=smtp://apikey:SECRET@smtp.example.com:587?tls=required   # STARTTLS
MAIL_FROM="My App <no-reply@example.com>"
APP_URL=https://app.example.com`}</code></pre>
        <p>Mail leaves through the job queue. A flow writes its message as a
        <code>mail.send</code> job on its own transaction, so a flow that fails
        sends nothing, and the job worker started with the server sends it right
        after the commit, and every 5 seconds whatever is due.</p>

        <h2>Adding a flow</h2>
        <p>
          A flow that mails a link needs a token purpose, a lifetime, a message
          and an endpoint. The steps are: add the purpose to the
          <code>auth_tokens</code> check in <code>schema/0002_auth.sql</code>
          and run <code>rok sqlc g</code>; add a method to <code>Templates</code>
          in <code>src/shared/mail/templates.rs</code>; add the lifetime in
          <code>src/shared/security/auth_token.rs</code>; write the service
          methods; add the endpoints in <code>handlers.rs</code>,
          <code>routes.rs</code> and <code>src/openapi.rs</code>.
        </p>

        <h2>Testing the flows</h2>
        <p>
          <code>tests/</code> drives each flow over HTTP against PostgreSQL (set
          <code>DATABASE_URL</code>; each test makes and drops its own schema).
          Helpers in <code>tests/support/mod.rs</code>:
          <code>call</code>, <code>mailed</code>,
          <code>register_verified</code>. Worked examples in
          <code>tests/sign_up.rs</code>, <code>login.rs</code>,
          <code>password.rs</code>, <code>email_change.rs</code>,
          <code>invitations.rs</code> and <code>mail.rs</code>.
        </p>
      </article>
    </div>
  )
}