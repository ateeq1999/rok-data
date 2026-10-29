export default function Footer() {
  const year = new Date().getFullYear()

  return (
    <footer className="mt-20 border-t border-[var(--line)] px-4 pb-14 pt-10 text-[var(--sea-ink-soft)]">
      <div className="page-wrap flex flex-col items-center justify-between gap-4 text-center sm:flex-row sm:text-left">
        <p className="m-0 text-sm">
          &copy; {year} The rok-data Contributors. MIT OR Apache-2.0.
        </p>
        <div className="flex flex-wrap items-center justify-center gap-4 text-sm">
          <a
            href="https://github.com/ateeq1999/rok-data"
            target="_blank"
            rel="noreferrer"
            className="no-underline transition hover:text-[var(--sea-ink)]"
          >
            GitHub
          </a>
          <a
            href="https://crates.io/crates/rok-data-cli"
            target="_blank"
            rel="noreferrer"
            className="no-underline transition hover:text-[var(--sea-ink)]"
          >
            crates.io: rok-data-cli
          </a>
          <a
            href="/llms.txt"
            className="no-underline transition hover:text-[var(--sea-ink)]"
          >
            llms.txt
          </a>
        </div>
        <p className="island-kicker m-0 inline-flex items-center gap-2">
          <img src="/logo.svg" alt="" width={20} height={20} className="h-5 w-5" />
          One schema. All changes.
        </p>
      </div>
    </footer>
  )
}
