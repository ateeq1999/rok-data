# ADR 0024: jobs and schedules on one Postgres queue

Date: 2026-10-29 · Status: accepted

The templates run background work on their own Postgres queue: a `jobs`
table claimed with `FOR UPDATE SKIP LOCKED`, woken by `LISTEN/NOTIFY`,
and enqueued on the caller's transaction, so a rolled-back flow enqueues
nothing. Jobs implement a typed `Job` trait (kind, serde payload, `run`,
timeout, `on_dead`). The mail outbox, which already worked this way, is
folded in: mail is the `mail.send` job, and `outbound_mail` goes away
(template migration `0007_jobs`). Crons are jobs too: every instance runs
the scheduler, and each tick inserts a job keyed `(kind, scheduled_for)`
with `ON CONFLICT DO NOTHING`, so exactly one replica enqueues it, with
no leader election or held lock; a missed tick runs once, without
catch-up. `APP_ROLE=all|web|worker` picks what a process runs.

## Considered Options

- **apalis** (Postgres backend): another crate's schema and release
  cadence inside user-owned code, and no enqueue on the flow's own
  transaction without working around it.
- **Redis**: a second datastore, against the PostgreSQL-only scope.
- **Leader election / advisory locks for crons**: a held lock per
  scheduler and failover rules, where a unique insert gives
  once-per-tick for free.

## Consequences

- Delivery stays at least once; jobs must be idempotent.
- Queue throughput is bounded by Postgres; a team that outgrows it swaps
  the queue behind the `Job` trait.
