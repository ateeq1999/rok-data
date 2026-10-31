import { Link, createFileRoute } from '@tanstack/react-router'
import { cn } from 'cn'
import { buttonVariants } from '../components/ui/button'
import { Card, CardDescription, CardHeader } from '../components/ui/card'

export const Route = createFileRoute('/')({ component: HomePage })

function HomePage() {
  return (
    <main className="page-wrap px-4 pb-8 pt-14">
      <Card className="gap-0 px-6 py-10 sm:px-10 sm:py-14">
        <p className="kicker mb-3">rok-data Documentation</p>
        <h1 className="mb-5 max-w-3xl text-4xl leading-[1.02] font-bold tracking-tight text-foreground sm:text-6xl">
          One schema. All changes.
        </h1>
        <p className="mb-8 max-w-2xl text-base text-muted-foreground sm:text-lg">
          PostgreSQL data tooling for Rust, consolidated from the rok
          ecosystem. The project's schema directory is the only source of
          truth — <code>rok init</code> scaffolds it, <code>rok gen</code>{' '}
          turns it into a typed data layer and an actix-web or axum API, and
          the CLI's machine contracts drive it from a coding agent.
        </p>
        <div className="flex flex-wrap gap-3">
          <Link
            to="/guide/installation"
            className={cn(buttonVariants({ size: 'lg' }), 'no-underline')}
          >
            Get Started
          </Link>
          <Link
            to="/guide/getting-started"
            className={cn(buttonVariants({ variant: 'outline', size: 'lg' }), 'no-underline')}
          >
            Quick Start
          </Link>
          <Link
            to="/agents"
            className={cn(buttonVariants({ variant: 'outline', size: 'lg' }), 'no-underline')}
          >
            For Agents
          </Link>
          <a
            href="https://github.com/ateeq1999/rok-data"
            target="_blank"
            rel="noopener noreferrer"
            className={cn(buttonVariants({ variant: 'outline', size: 'lg' }), 'no-underline')}
          >
            GitHub
          </a>
        </div>
      </Card>

      <section className="mt-8 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {[
          [
            'Typed models',
            'One derive gives typed column constants, relations, scopes and the trait impls — reads chain, writes row-level.',
          ],
          [
            'Migrations with drift detection',
            'Numbered up/down pairs, a checksummed journal (advisory-locked, transactional), states applied|pending|edited|unknown.',
          ],
          [
            'One front door',
            'rok init scaffolds the project, rok gen generates the data layer and API features, staged pairs keep the schema history.',
          ],
          [
            'Two generators, one engine pair',
            'A whole-crate generator and the resource-module engine — legacy `rok generate` / `rok sqlc …` keep working until 1.0.',
          ],
          [
            'Actix-web or axum',
            'API features and complete starter templates for both frameworks, with OpenAPI (utoipa) and a check-arch layout gate.',
          ],
          [
            'Agent-native contracts',
            '--json shapes, exit codes, --dry-run and check modes — plus in-repo agent skills under .agents/skills/rok-data*/.',
          ],
        ].map(([title, desc]) => (
          <Card key={title}>
            <CardHeader>
              <h2 className="m-0 text-base font-semibold text-card-foreground">
                {title}
              </h2>
              <CardDescription className="text-sm">{desc}</CardDescription>
            </CardHeader>
          </Card>
        ))}
      </section>

      <Card className="mt-8 grid gap-0 py-0 md:grid-cols-2">
        <div className="p-6">
          <p className="kicker mb-2">The workspace</p>
          <h2 className="mb-4 text-xl font-semibold text-foreground">
            Core Crates
          </h2>
          <ul className="m-0 space-y-2 text-sm text-muted-foreground">
            <li><Link to="/crates/rok-data" className="text-primary hover:underline">rok-data</Link> — the facade applications depend on</li>
            <li><Link to="/crates/rok-data-core" className="text-primary hover:underline">rok-data-core</Link> — Db, Expr/Select, migrations, introspect</li>
            <li><Link to="/crates/rok-data-macros" className="text-primary hover:underline">rok-data-macros</Link> — derives + #[rok_data::test]</li>
            <li><Link to="/crates/rok-data-codegen" className="text-primary hover:underline">rok-data-codegen</Link> — whole-crate generator</li>
            <li><Link to="/crates/rok-data-sqlc" className="text-primary hover:underline">rok-data-sqlc</Link> — resource-module engine</li>
            <li><Link to="/crates/rok-templates" className="text-primary hover:underline">rok-templates</Link> — production actix/axum templates</li>
            <li><Link to="/crates/rok-cli" className="text-primary hover:underline">rok (rok-data-cli)</Link> — the one binary</li>
          </ul>
        </div>

        <div className="p-6">
          <p className="kicker mb-2">Quick Links</p>
          <h2 className="mb-4 text-xl font-semibold text-foreground">
            Resources
          </h2>
          <ul className="m-0 space-y-2 text-sm text-muted-foreground">
            <li><Link to="/guide/getting-started" className="text-primary hover:underline">Getting Started</Link></li>
            <li><Link to="/guide/templates" className="text-primary hover:underline">Templates Guide</Link></li>
            <li><Link to="/agents" className="text-primary hover:underline">Coding Agents (JSON contracts)</Link></li>
            <li><a href="https://github.com/ateeq1999/rok-data/tree/main/docs" className="text-primary hover:underline" target="_blank" rel="noreferrer">docs/ (AI-friendly markdown)</a></li>
            <li><a href="https://raw.githubusercontent.com/ateeq1999/rok-data/main/docs/llms.txt" className="text-primary hover:underline" target="_blank" rel="noreferrer">docs/llms.txt</a></li>
            <li><a href="https://crates.io/crates/rok-data-cli" className="text-primary hover:underline" target="_blank" rel="noreferrer">crates.io</a></li>
          </ul>
        </div>
      </Card>

      <Card className="mt-8 gap-0 p-6">
        <p className="kicker mb-2">Example</p>
        <h2 className="mb-4 text-xl font-semibold text-foreground">
          One schema. All changes.
        </h2>
        <pre className="text-xs"><code>{`$ rok init my-api --schema schema.sql
$ cd my-api && rok gen          # src/db/** + src/features/**
$ rok gen --migration add_posts # stages 0002_add_posts.{up,down}.sql
$ rok gen check                 # CI: exit 1 when the code is stale
$ rok migrate up                # apply, journal-checked`}</code></pre>
      </Card>
    </main>
  )
}
