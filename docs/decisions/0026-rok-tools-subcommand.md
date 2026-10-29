# ADR 0026: `rok tools …` forwards to rok-tools

Date: 2026-10-29 · Status: accepted (amends ADR 0002 and ADR 0011)

## Context

rok-tools is a separate workspace that ships developer Tools (grep,
replace, scan, loc, …) as a feature-gated library plus its own
`rok-tools` binary. Users of the `rok` binary should reach the same
Tools without installing a second CLI.

## Decision

- ADR 0002's command set gains `tools …`.

- Like `rok sqlc …`, tokens are not re-parsed: they are forwarded
  verbatim to `rok_tools_cli::run_from`, so `rok tools <tool>` and
  `rok-tools <tool>` share one clap grammar, help text and exit codes.
  The root help embeds `rok_tools_cli::command().name("tools")` purely
  so `rok --help` lists it.

- `rok-data-cli` gates this behind a `tools` cargo feature, on by
  default, which pulls in `rok-tools-cli` with its default features:
  every Tool (`rok-tools/full`, so `bash`/`http` included), the Payload
  Runner (`rok tools run`), and `[tools.<tool>]` settings read from the
  same `rok.toml` rok-data uses.

- ADR 0011's section list gains `[tools]`, owned by rok-tools. Both
  rok-data readers of `rok.toml` (the CLI's and the engine's) accept and
  ignore it, and still deny every other unknown section; rok-tools reads
  only `[tools]`.

- `--json` and exit codes follow ADR 0002: usage errors 2; runtime
  failures and check violations (e.g. `rok tools loc --check`) 1.

## Consequences

- `rok-data-cli` takes a dependency on `rok-tools-cli`; building with
  `--no-default-features` drops it. Until rok-tools is published, the
  dependency is a path to a sibling `../rok-tools` checkout.
