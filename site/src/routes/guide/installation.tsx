import { Link, createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/guide/installation')({
  component: InstallationGuide,
})

function InstallationGuide() {
  return (
    <div className="mx-auto max-w-4xl px-4 py-8">
      <article className="prose max-w-none">
        <h1>Installation</h1>
        <p className="lead">
          The rok-data toolchain in minutes: one binary (<code>rok</code>),
          one runtime crate (<code>rok-data</code>), one PostgreSQL server.
        </p>

        <h2>Prerequisites</h2>
        <ul>
          <li>Rust <strong>1.94</strong> or later — edition 2024</li>
          <li>PostgreSQL <strong>12+</strong> for anything that touches a
            database (generation itself needs no database)</li>
          <li>Name note: the CLI crate publishes as
            <strong> rok-data-cli</strong>; the binary it provides is
            <strong> rok</strong></li>
        </ul>

        <h2>Install the CLI</h2>
        <pre><code>cargo install rok-data-cli</code></pre>
        <p>From a checkout of this repository instead:</p>
        <pre><code>cargo install --path crates/rok-cli</code></pre>
        <p>
          Or take a prebuilt binary: GitHub releases carry checksummed
          archives for Linux (musl), macOS and Windows, x86_64 and ARM64,
          and the repo's <code>install.sh</code> / <code>install.ps1</code>
          download, verify and install them.
        </p>

        <h2>Verify</h2>
        <pre><code>rok --version</code></pre>
        <p>Should print <code>rok 0.3.0</code> or later.</p>

        <h2>Add the runtime</h2>
        <pre><code>{`cargo new my-app && cd my-app
cargo add rok-data`}</code></pre>
        <p>
          The <code>rok-data</code> facade is the only dependency
          applications need: it re-exports the runtime
          (<code>rok-data-core</code>) and the macros
          (<code>rok-data-macros</code>). Point the toolchain at a database
          with <code>--database-url</code> or the <code>DATABASE_URL</code>
          environment variable; URLs are always printed masked.
        </p>
        <pre><code>export DATABASE_URL=postgres://postgres:postgres@localhost:5432/my_app</code></pre>

        <h2>Platform-Specific Notes</h2>

        <h3>Windows</h3>
        <p>
          Ensure you have the Microsoft C++ Build Tools installed.
          Download from:{' '}
          <a href="https://visualstudio.microsoft.com/visual-cpp-build-tools/">
            Visual Studio Build Tools
          </a>
          , then run <code>cargo</code> from a fresh terminal.
        </p>

        <h3>macOS</h3>
        <p>Make sure Xcode Command Line Tools are installed:</p>
        <pre><code>xcode-select --install</code></pre>

        <h3>Linux</h3>
        <p>Build dependencies only; the runtime needs none:</p>
        <pre><code>{`# Ubuntu/Debian
sudo apt-get install build-essential pkg-config libssl-dev

# Fedora
sudo dnf install pkg-config openssl-devel

# Arch
sudo pacman -S pkg-config openssl`}</code></pre>

        <h2>Next Steps</h2>
        <ul>
          <li><Link to="/guide/getting-started">Getting Started — the front door</Link></li>
          <li><Link to="/crates/rok-cli">the rok binary — commands and flags</Link></li>
          <li><Link to="/agents">For coding agents — JSON contracts and exit codes</Link></li>
        </ul>
      </article>
    </div>
  )
}
