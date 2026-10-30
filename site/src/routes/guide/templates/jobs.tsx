import { Link, createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/guide/templates/jobs')({
  component: TemplatesJobsGuide,
})

function TemplatesJobsGuide() {
  return (
    <div className="mx-auto max-w-4xl px-4 py-8">
      <article className="prose dark:prose-invert max-w-none">
        <h1>Jobs and schedules</h1>
        <p>
          Work that need not happen inside the request — sending mail, building
          a report, calling a slow API — runs as a <strong>job</strong> on
          PostgreSQL itself (<code>src/shared/jobs/</code>, workspace ADR
          0024). There is no other datastore to run, and a job is written on
          the same transaction as the change that asks for it, so a rolled-back
          request enqueues nothing.
        </p>

        <h2>How it works</h2>
        <p>
          A job is a row in <code>jobs</code>: its kind, its payload (JSON),
          its status (<code>pending</code> or <code>dead</code>), its attempts
          and when it is next due. The worker running beside the server claims
          the due job waiting longest with <code>FOR UPDATE SKIP LOCKED</code>
          — two workers, in one process or many, never take the same row —
          runs it, and deletes it, all in the claiming transaction. A failed
          attempt is retried after 30 seconds, then 1, 2 and 4 minutes; after
          its kind's last attempt (5 by default) the job is <code>dead</code>,
          kept with its <code>last_error</code>, never tried again.
        </p>
        <p>The worker wakes as soon as a job is committed: in-process at once,
        from another process through <code>NOTIFY jobs</code>, and every 5
        seconds otherwise.</p>
        <p>
          Delivery is <strong>at least once</strong>: a crash after a job ran
          but before its claim committed runs it again. Make each job safe to
          repeat (an idempotency key on the call it makes, a check that the
          work is not done already). A running job holds its claim — one pool
          connection — until it settles, so keep <code>JOBS_CONCURRENCY</code>
          well under <code>DB_MAX_CONNECTIONS</code>.
        </p>
        <p>
          A job's span (<code>job</code>, with its kind, id and attempt)
          continues the trace of the request that enqueued it, so a trace
          backend shows the request and the mail it sent as one trace.
        </p>

        <h2>Adding a job</h2>
        <p>A job is a payload type and what to do with it. Mail is the first one:</p>
        <pre><code>{`#[async_trait]
impl Job for SendMail {
    const KIND: &'static str = "mail.send";
    const MAX_ATTEMPTS: i32 = MAX_ATTEMPTS;
    type Context = Arc<dyn Mailer>;

    async fn run(self, mailer: &Arc<dyn Mailer>) -> Result<(), JobError> {
        mailer.send(&self.0).await?;
        Ok(())
    }

    /// A dead mail's bodies are dropped, since a link they carry must not
    /// outlive the attempts to send it.
    fn on_dead(mut payload: serde_json::Value) -> serde_json::Value {
        for body in ["text", "html"] {
            if let Some(value) = payload.get_mut(body) {
                *value = serde_json::Value::from(REMOVED);
            }
        }
        payload
    }
}`}</code></pre>
        <ol>
          <li><strong>The payload</strong>, <code>#[derive(Serialize, Deserialize)]</code>: what the job needs to know, small (ids, not rows). It is stored as JSON until the job runs.</li>
          <li><strong><code>impl Job</code></strong>: <code>KIND</code> names it in the table and the metrics (never rename one with jobs queued); <code>Context</code> is what <code>run</code> needs, such as a service; <code>run</code> does the work, and any error converts with <code>?</code>. <code>MAX_ATTEMPTS</code>, <code>TIMEOUT</code> (5 minutes) and <code>on_dead</code> (what a dead job's payload keeps; strip what must not outlive the attempts) have defaults. The job's type lives in the feature that owns the work, in its <code>service.rs</code>.</li>
          <li><strong>Register it</strong> in <code>app/run.rs</code>'s <code>worker</code>, with its context:
            <pre><code>{`pub fn worker(state: &AppState, mailer: Arc<dyn Mailer>) -> Worker {
    Worker::new(state.pool.clone(), &state.queue)
        .with_concurrency(state.jobs_concurrency)
        .register::<SendMail>(mailer)
        .register::<SweepUploads>(MediaService::clone(&state.media))
}`}</code></pre>
            A job of a kind no worker registers fails at once and is <code>dead</code>, so register every kind in this one place.
          </li>
          <li><strong>Enqueue it</strong> on the request's transaction, from the service that holds a <code>Queue</code> (<code>AppState.queue</code>): <code>queue.enqueue(&mut tx, &job)</code>, then <code>queue.commit(tx)</code>, which commits and wakes the worker. <code>{"queue.enqueue_with(&mut tx, &job, Enqueue { delay, unique_key })"}</code> delays it, or enqueues it only when no job holds the same <code>unique_key</code> (it answers <code>false</code> then).</li>
        </ol>

        <h2>Scheduled jobs</h2>
        <p>A job on a clock is one line of <code>app/run.rs</code>'s <code>schedule</code>:</p>
        <pre><code>{`pub fn schedule() -> Result<Schedule, ScheduleError> {
    Schedule::new().cron::<SweepUploads>("*/5 * * * *")
}`}</code></pre>
        <p>
          <code>.cron::&lt;Kind&gt;(pattern)?</code> enqueues <code>Kind::default()</code>
          at every tick of a cron pattern (<code>minute hour day month weekday</code>,
          UTC, <code>croner</code>'s syntax); the kind must be registered in
          <code>worker</code> too. A pattern that doesn't parse stops the
          start-up, naming the kind.
        </p>
        <p>
          Every process running jobs runs the schedule, and each tick runs once
          however many there are: at a tick, each tries to move the kind's
          <code>last_tick</code> forward in <code>job_schedules</code>, and
          the row lock lets one transaction win — that one enqueues the job. A
          tick missed while nothing ran (a deploy, an outage) runs once when a
          process is back, not once per missed tick; a schedule seen for the
          first time waits for its next tick rather than running at once, so a
          nightly job does not fire at the first boot.
        </p>

        <h2>Processes: <code>APP_ROLE</code></h2>
        <p>One image runs any part of the application:</p>
        <table>
          <thead>
            <tr><th><code>APP_ROLE</code></th><th>Serves</th><th>Runs jobs and the schedule</th></tr>
          </thead>
          <tbody>
            <tr><td><code>all</code> (the default)</td><td>the API</td><td>yes</td></tr>
            <tr><td><code>web</code></td><td>the API</td><td>no: its jobs run in a <code>worker</code></td></tr>
            <tr><td><code>worker</code></td><td><code>/health/live</code> and <code>/health/ready</code> only</td><td>yes</td></tr>
          </tbody>
        </table>
        <p>Run <code>all</code> in one process while it is small; split into
        <code>web</code> (scaled with traffic) and <code>worker</code> (scaled
        with <code>{"jobs_queued{state=\"due\"}"}</code>) when it is not. On
        <code>SIGTERM</code> the worker stops taking jobs; a job still running
        past <code>SHUTDOWN_GRACE_SECS</code> is abandoned, its transaction
        rolled back, so it runs again on another worker, no attempt counted.</p>

        <h2>Failed jobs</h2>
        <p>A dead job stays in <code>jobs</code> until someone acts. The
        <code>jobs</code> feature (<code>src/features/jobs/</code>, admins
        only) is that someone:</p>
        <ul>
          <li><code>GET /api/v1/jobs?status=dead&kind=mail.send</code> lists
            them, newest first, paginated. A job's payload is never answered:
            a pending mail's holds its link.</li>
          <li><code>{"POST /api/v1/jobs/{id}/retry"}</code> puts a dead job back
            in the queue, its attempts at zero.</li>
          <li><code>{"DELETE /api/v1/jobs/{id}"}</code> cancels a pending or dead
            job; <code>409</code> while a worker is running it.</li>
        </ul>
        <p>Retries and cancellations are in the audit log (<code>job.retried</code>,
        <code>job.cancelled</code>). Alert on
        <code>{"jobs_queued{state=\"dead\"} > 0"}</code>.</p>

        <h2>Testing jobs</h2>
        <p>Tests run jobs without a background task and without sleeping:
        <code>Worker::run_due()</code> runs every due job, one after another,
        until none is due (a failing job with
        <code>with_backoff(Duration::ZERO)</code> runs until it is dead), and
        answers how many ran and failed. Schedules are driven with the clock
        a test wants: <code>schedule.tick_at(&pool, &queue, now)</code>.
        <code>tests/jobs.rs</code> and <code>tests/schedule.rs</code> show
        both.</p>

        <h2>Settings</h2>
        <table>
          <thead>
            <tr><th>Variable</th><th>Default</th><th></th></tr>
          </thead>
          <tbody>
            <tr><td><code>JOBS_CONCURRENCY</code></td><td>4</td><td>jobs a worker runs at once; each holds a pool connection</td></tr>
            <tr><td><code>APP_ROLE</code></td><td><code>all</code></td><td>what the process runs</td></tr>
            <tr><td><code>SHUTDOWN_GRACE_SECS</code></td><td>25</td><td>how long running jobs have to finish on <code>SIGTERM</code></td></tr>
          </tbody>
        </table>
      </article>
    </div>
  )
}