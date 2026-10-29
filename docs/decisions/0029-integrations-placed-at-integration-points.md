# ADR 0029: Integrations are placed at Integration points; Blueprints save a choice

Date: 2026-10-29 · Status: accepted (amends ADR 0023)

ADR 0023 stands for Capabilities: compiled in, inert until the
environment turns them on, one code path. What it left as written
recipes — **Integrations**, the code that picks a vendor or an opt-in
dependency (a Sentry exporter, a Redis cache, a RabbitMQ broker) — a
command now places, at **Integration points** the Template marks in
advance. ADR 0023 rejected a post-init `add` because it would have to
patch files the user already owns by guessing; a marked point removes
the guess.

## Decision

- A point is a comment in the file's own syntax, kept forever so several
  Integrations can stack on it; what a command places is fenced by
  name:

  ```rust
  // rok:point tracing.exporters
  // rok:begin sentry
  let sentry = sentry_tracing::layer();
  // rok:end sentry
  ```

  The fence makes placing twice a no-op and removal possible; the code
  inside it is the user's to edit. Points are not `@generated` headers
  (ADR 0005): the file around them stays user-owned.

- `rok add <integration>` (`--list`, `--dry-run` prints the diff) and
  `rok remove <integration>`, which refuses on an edited fence unless
  `--force`. A missing point is an error naming the file and the point,
  and nothing is written unless every point is found. Dependencies go
  through points in `Cargo.toml`, `package.json` and `pubspec.yaml`.

- Integrations are data in rok-templates' catalog (ADR 0027): the
  Templates and points they target, their snippets, their dependencies.
  Gates: each Integration applied alone, compiled and tested; one build
  with all applied; `remove` restoring the Template byte for byte —
  n + 1 builds, not the 2ⁿ ADR 0023 rejected.

- A **Blueprint** saves a Template, its Integrations and init options
  under an id (`rok blueprint save acme-saas`,
  `rok init myapp --blueprint acme-saas`). `--blueprint` also takes a
  file path or an https URL to a `.toml`, which is how a team shares
  one; there is no hosted registry.

## Consequences

- Tracing, metrics and the other Capabilities need no Integration;
  sending traces to a vendor is the vendor's Integration on the
  Capability's exporter point.
- A user who deletes a point loses `rok add` for the Integrations that
  need it, by design, with an error saying which.
