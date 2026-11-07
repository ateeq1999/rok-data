# 03: Owning children, and their logs

Spec: `docs/specs/rok-dev.md` · workspace ADR 0034

**What to build:** a supervisor that starts each Runnable in a Job Object (Windows, kill-on-close) or its own process group (Unix), stops the whole tree on Ctrl-C, a closed terminal, SIGTERM or the parent's death, multiplexes output line by line with a coloured, aligned `name │` prefix, and with `--json` prints one event per line (`started`, `building`, `ready`, `exited`).

**Blocked by:** 02

**Status:** blocked

- [ ] Killing `rok dev` leaves no child running, on Windows and Unix (integration test with helper binaries)
- [ ] Prefixes and `--json` events (tests)
