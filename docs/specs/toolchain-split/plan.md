# Toolchain split: runbook

Splits rok-data into member repos under one `rok` command, run by hand
from PowerShell with `git` and `gh`. The why is
[ADR 0032](../../decisions/0032-member-repos-and-subcommand-clis.md);
this page is the how.

## End state

```text
D:\DEV\open-source\PROJECT_ROK\        ← rok-workspace (meta-repo; ignores the folders below)
├── rok-data\        runtime, codegen, sqlc engine, its CLI (binary rok-data → later rok-db + rok-gen)
├── rok-templates\   actix, axum and monorepo templates + their gates
├── rok-tools\       Tools library + rok-tools-cli (published)
├── rok-ui\          unchanged
└── rok-v2\          the `rok` binary (crate rok-cli 0.7.0): routing, help, PATH fallback, releases
```

**How to read it.** Each step ends with **Verify**: run it, compare, and
stop if it does not match. Steps marked **Agent task** are code changes:
hand the task text to an agent in this window and run its **Accept**
commands yourself. **⚠ Irreversible** marks the only steps that cannot
be undone (crates.io publishes; deleting a folder whose bundle you did
not verify).

**Set these once per PowerShell session:**

```powershell
$Root    = "D:\DEV\open-source\PROJECT_ROK"
$Archive = "D:\DEV\_archive"
$Gh      = "ateeq1999"
```

**Facts this plan was written against (2026-11-02):** `gh` 2.103 logged in
as `ateeq1999` with `repo` and `workflow` scopes; no Python, so history
is carried with `git subtree split`, not `git filter-repo`; crates.io:
`rok-cli` (0.6.1, the old runner), `rok-data-cli`, `rok-templates`,
`rok-db` are ours, `rok-tools`, `rok-tools-cli`, `rok-db-cli`,
`rok-gen-cli` are free.

---

## Step 0 — Back up everything first

Local state found on 2026-11-02:

| Repo | Remote | Unpushed commits | Uncommitted files |
| --- | --- | --- | --- |
| auth-starter | **none** | 4 | 0 |
| rok | ateeq1999/rok | 0 | 2 |
| rok-data | ateeq1999/rok-data | 3 (`production-templates`) | 1 |
| rok-db | ateeq1999/rok-db | 0 | 0 |
| rok-sqlc | ateeq1999/rok-sqlc | 1 | 0 |
| rok-ui-hooks | ateeq1999/rok-ui-hooks | 0 | 12 |
| rok.rs | better-auth-rs (not ours) | 0 | 0 |
| t09-scratch | not a git repo | — | — |

**0a. Commit the loose work onto a branch, so it lands in the bundle and
on GitHub.** Re-run `git status` first; the table is a snapshot.

```powershell
Set-Location $Root
foreach ($r in "rok", "rok-ui-hooks") {
    git -C $r status --short
    git -C $r switch -c wip/pre-archive
    git -C $r add -A
    git -C $r commit -m "WIP saved before archiving"
    git -C $r push -u origin wip/pre-archive
}
git -C rok-sqlc push origin --all
```

rok-data's `production-templates` branch: finish and push it as usual
(`git -C rok-data push`) — the later steps start from an up-to-date
`main`.

**0b. Bundle every repo** (a bundle is a full clone in one file; restore
with `git clone x.bundle x`). rok-data, rok-tools and rok-ui are bundled
too, as the rollback point before the surgery.

```powershell
New-Item -ItemType Directory -Force $Archive | Out-Null
$all = "auth-starter","rok","rok-data","rok-db","rok-sqlc","rok-tools","rok-ui","rok-ui-hooks","rok.rs"
foreach ($r in $all) { git -C "$Root\$r" bundle create "$Archive\$r.bundle" --all }
Compress-Archive -Path "$Root\t09-scratch" -DestinationPath "$Archive\t09-scratch.zip"
```

**0c. Copy what git ignores** (`.env`, local config): a bundle holds only
commits.

```powershell
foreach ($r in $all) { "== $r"; git -C "$Root\$r" status --ignored --short | Select-String '^!!' | Select-String -NotMatch 'target/|node_modules/|\.dart_tool/|build/' }
```

Copy anything listed that you want into `$Archive\<repo>-ignored\`.

### Verify

```powershell
foreach ($r in $all) { git bundle verify -q "$Archive\$r.bundle"; "$r -> $LASTEXITCODE" }   # every line ends in 0
git clone -q "$Archive\auth-starter.bundle" "$env:TEMP\auth-check"; git -C "$env:TEMP\auth-check" log --oneline -4   # the 4 local-only commits
Remove-Item -Recurse -Force "$env:TEMP\auth-check"
```

---

## Step 1 — Archive the old remotes, delete the local repos

**1a. Point the old `rok` repo at its successor** before it goes
read-only (`cargo install rok-cli` will install rok-v2 from 0.7.0):

```powershell
Set-Location "$Root\rok"; git switch main; git pull
# Put this at the top of README.md, then:
#   > **Archived.** The `rok` command now lives in https://github.com/ateeq1999/rok-v2
#   > (crate `rok-cli` from 0.7.0). This repository's task runner and crates
#   > stop at their last release.
git commit -am "Archive: point to rok-v2"; git push
```

**1b. Archive on GitHub** (read-only, links keep working;
`gh repo unarchive` reverses it). Never delete.

```powershell
foreach ($r in "rok", "rok-db", "rok-sqlc", "rok-ui-hooks") { gh repo archive "$Gh/$r" --yes }
```

`auth-starter` has no remote and lives on in its bundle; `rok.rs` is
upstream's, so nothing happens on GitHub.

**1c. Delete the local folders.** ⚠ Irreversible locally: run only after
step 0's Verify printed `0` for every bundle.

```powershell
Set-Location $Root
foreach ($r in "auth-starter","rok","rok-db","rok-sqlc","rok-ui-hooks","rok.rs","t09-scratch") { Remove-Item -Recurse -Force "$Root\$r" }
```

### Verify

```powershell
(Get-ChildItem $Root -Directory).Name          # rok-data, rok-tools, rok-ui
gh repo list $Gh --archived --limit 50 --json name -q '.[].name'   # includes rok, rok-db, rok-sqlc, rok-ui-hooks
```

`cargo-clean.sh` at the root moves into the meta-repo's `scripts\` in
step 2.

---

## Step 2 — Make `PROJECT_ROK` the meta-repo, `rok-workspace`

**2a. Initialize, ignoring everything but the meta files** (so a newly
cloned member is never committed by accident):

```powershell
Set-Location $Root
git init -b main
@'
/*
!/.gitignore
!/README.md
!/AGENTS.md
!/CLAUDE.md
!/GLOSSARY-MAP.md
!/repos.toml
!/.cargo/
!/.github/
!/docs/
!/scripts/
'@ | Set-Content -Encoding utf8 .gitignore
```

**2b. The member list** — the one file both `sync.ps1` and CI read:

```powershell
@'
# Member repos, cloned side by side into this folder. name = "clone URL"
[repos]
rok-data = "https://github.com/ateeq1999/rok-data.git"
rok-tools = "https://github.com/ateeq1999/rok-tools.git"
rok-ui = "https://github.com/ateeq1999/rok-ui.git"
'@ | Set-Content -Encoding utf8 repos.toml
```

(`rok-templates` joins in step 4, `rok-v2` in step 6.)

**2c. `scripts\sync.ps1`** — clone what is missing, fast-forward the rest:

```powershell
New-Item -ItemType Directory -Force scripts | Out-Null
Move-Item cargo-clean.sh scripts\
@'
# Clone every member repo in repos.toml that is missing; fast-forward the rest.
$root = Split-Path $PSScriptRoot
foreach ($line in Get-Content "$root\repos.toml") {
    if ($line -match '^\s*([\w.-]+)\s*=\s*"([^"]+)"') {
        $dir = Join-Path $root $Matches[1]
        if (Test-Path $dir) { git -C $dir pull --ff-only } else { git clone $Matches[2] $dir }
    }
}
'@ | Set-Content -Encoding utf8 scripts\sync.ps1
```

**2d. The cross-repo patch file**, used only when asked for
(`--config`), never picked up implicitly (ADR 0032):

```powershell
New-Item -ItemType Directory -Force .cargo | Out-Null
@'
# Build a member against the other members' checkouts instead of crates.io:
#   cargo build --config ..\.cargo\patch-local.toml
# Paths are relative to this folder's parent (the workspace root).
[patch.crates-io]
rok-data-core    = { path = "rok-data/crates/rok-data-core" }
rok-data-macros  = { path = "rok-data/crates/rok-data-macros" }
rok-data         = { path = "rok-data/crates/rok-data" }
rok-data-codegen = { path = "rok-data/crates/rok-data-codegen" }
rok-data-sqlc    = { path = "rok-data/crates/rok-data-sqlc" }
rok-data-cli     = { path = "rok-data/crates/rok-cli" }
rok-tools        = { path = "rok-tools/crates/rok-tools" }
rok-tools-cli    = { path = "rok-tools/crates/rok-tools-cli" }
'@ | Set-Content -Encoding utf8 .cargo\patch-local.toml
```

(`rok-templates` is added in step 4.)

**2e. Agent task W1 — the meta-repo's docs.** Write, in `$Root`:
`README.md` (what this folder is, `scripts\sync.ps1`, the patch file);
`AGENTS.md` (the member repos table from "End state", where each kind of
change belongs, the patch-file recipe, the toolchain ADR series cited as
"toolchain ADR NNNN" under `docs/decisions/`, a pointer to each member's
own AGENTS.md); `CLAUDE.md` pointing at `AGENTS.md`;
`GLOSSARY-MAP.md` linking each member's glossary and holding the
toolchain terms (Member repo, Meta-repo, Subcommand CLI) moved out of
rok-data's `GLOSSARY.md`. Move this plan to `docs\plan.md`.
**Accept:** `git status --short` lists only the meta files; no member
folder appears.

**2f. Publish:**

```powershell
Move-Item rok-data\docs\specs\toolchain-split\plan.md docs\plan.md   # if W1 did not
git add -A; git commit -m "rok-workspace: the meta-repo of the rok toolchain"
gh repo create "$Gh/rok-workspace" --public --source . --push --description "The rok toolchain's member repos, side by side"
```

The move leaves `docs/specs/toolchain-split/plan.md` deleted in rok-data;
A1 (step 3) commits that deletion and repoints `docs/llms.txt` /
`docs/README.md` at `https://github.com/ateeq1999/rok-workspace/blob/main/docs/plan.md`.

### Verify

```powershell
git ls-files                                   # only meta files; no rok-data/, rok-tools/, rok-ui/
& "$Root\scripts\sync.ps1"                     # "Already up to date." for each member
```

---

## Step 3 — rok-data: free the `rok` binary name

**Agent task A1** (in `rok-data`, branch `toolchain-split/binary-name`):

1. `crates/rok-cli/Cargo.toml`: `[[bin]] name = "rok"` → `"rok-data"`;
   every `env!("CARGO_BIN_EXE_rok")` in `crates/rok-cli/tests` (25
   uses, 23 files) → `CARGO_BIN_EXE_rok-data`; expected help output that
   prints the program name follows.
2. Add `pub fn command() -> clap::Command` beside `pub fn run` in
   `crates/rok-cli/src/lib.rs`, so `rok-v2` can embed the help (ADR 0026's
   pattern).
3. AGENTS.md: invariant 2 ("one binary target: `rok-data`; the `rok`
   binary is rok-v2's"), invariant 4 (`CARGO_BIN_EXE_rok-data`), the
   Releasing section's wrong claim that `rok-cli` belongs to another
   crate (it is ours: the old runner, then rok-v2), and the ADR range
   (0032). `docs/cli-reference.md`, README and `docs/getting-started.md`:
   the standalone binary is `rok-data`; `rok` comes from rok-v2.
4. CHANGELOG `## [0.6.0]` entry: the binary rename.

`release.yml` keeps building binaries until rok-v2 ships its own
(step 6e), so there is no release without an installable binary.

**Accept:**

```powershell
Set-Location "$Root\rok-data"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo run -q -p rok-data-cli -- --help         # prints "Usage: rok-data ..."
```

**Ship it:**

```powershell
git push -u origin HEAD
gh pr create --fill --base main
gh pr checks --watch
gh pr merge --squash --delete-branch
git switch main; git pull
```

---

## Step 4 — Extract `rok-templates` into its own repo

**4a. Split the history out** (subtree split carries every commit that
touched `crates/rok-templates` since ADR 0022 moved it there; earlier
history stays in rok-data and its bundle). It takes several minutes.

```powershell
Set-Location "$Root\rok-data"; git switch main; git pull
git subtree split --prefix=crates/rok-templates -b templates-split
gh repo create "$Gh/rok-templates" --public --description "Starter templates for the rok toolchain: actix and axum APIs, and an api + web + mobile + desktop monorepo"
git push "https://github.com/$Gh/rok-templates.git" templates-split:main
git branch -D templates-split
Set-Location $Root; git clone "https://github.com/$Gh/rok-templates.git"
```

**4b. Agent task T1** (in `rok-templates`, branch `standalone`): make it
build on its own.

1. `Cargo.toml`: replace every `*.workspace = true` with the values from
   rok-data's `[workspace.package]` (version stays `0.5.0`, repository
   and homepage become `https://github.com/ateeq1999/rok-templates`);
   add a root `[workspace]` with `exclude = ["templates/actix-api",
   "templates/axum-api"]`; copy rok-data's `[workspace.lints]` into
   `[lints]`. Copy `LICENSE-MIT`, `LICENSE-APACHE`, `rustfmt.toml`.
2. Move the template gates from rok-data
   (`crates/rok-cli/tests/fixture_compile/{actix_template,axum_template,monorepo_template,web_app,mobile_app,desktop_app,dart_client}.rs`,
   `dart_client_test.dart`, and the helpers they use) into
   `tests/`, generating with the `rok-data` binary from
   `cargo install rok-data-cli --version =0.6.0` (the pin; the gates say
   which version they expect and skip with a printed note when the
   binary is missing).
3. `.github/workflows/ci.yml`: fmt, clippy, `cargo test --all-targets`;
   a `gates` job that installs the pinned `rok-data-cli`, bun and
   Flutter, then runs the gates.
4. `.github/workflows/release.yml`: on tag `vX.Y.Z` equal to the crate
   version and a `## [X.Y.Z]` CHANGELOG heading, `cargo publish`.
5. `README.md`, `CHANGELOG.md`, `AGENTS.md` (where a change belongs; the
   templates' ADRs stay in rok-data's `docs/decisions/` 0022–0025, 0027,
   0029–0031 and are cited from there).

**Accept:**

```powershell
Set-Location "$Root\rok-templates"
cargo fmt --all -- --check; cargo clippy --all-targets -- -D warnings; cargo test --all-targets
cargo publish --dry-run                        # packages; no workspace-inheritance error
```

```powershell
gh secret set CARGO_REGISTRY_TOKEN --repo "$Gh/rok-templates"   # paste a crates.io token
git push -u origin HEAD; gh pr create --fill --base main; gh pr checks --watch; gh pr merge --squash --delete-branch
```

**4c. Agent task A2** (in `rok-data`, branch `toolchain-split/templates-out`):
remove `crates/rok-templates` and its `members`/`exclude` entries;
`[workspace.dependencies] rok-templates = "0.5.0"` (crates.io, no
path); delete the gates T1 moved; `release.yml` stops publishing
`rok-templates`; AGENTS.md's "Where changes belong" rows for templates
point to the rok-templates repo; `GLOSSARY-MAP.md` links its glossary
by URL; `docs/llms.txt` and `docs/README.md` follow. CHANGELOG `[0.6.0]`
gains the move.

**Accept:** step 3's four commands, plus
`cargo tree -p rok-data-sqlc -i rok-templates` showing
`rok-templates v0.5.0` with no local path.

**4d. Register the new member and release rok-data 0.6.0:**

```powershell
Set-Location $Root
Add-Content repos.toml 'rok-templates = "https://github.com/ateeq1999/rok-templates.git"'
Add-Content .cargo\patch-local.toml 'rok-templates    = { path = "rok-templates" }'
git commit -am "Add rok-templates"; git push

Set-Location "$Root\rok-data"; git switch main; git pull
git tag v0.6.0; git push origin v0.6.0
gh run watch (gh run list --workflow release.yml --limit 1 --json databaseId -q '.[0].databaseId')
```

### Verify

```powershell
cargo install rok-data-cli --version =0.6.0 --locked
rok-data --version                             # rok-data 0.6.0
gh workflow run ci.yml --repo "$Gh/rok-templates"; gh run watch --repo "$Gh/rok-templates" (gh run list --repo "$Gh/rok-templates" --limit 1 --json databaseId -q '.[0].databaseId')   # gates green against the pin
(git -C "$Root\rok-data" ls-files).Count       # ≈ 1440, down from 2718
```

---

## Step 5 — Publish rok-tools

⚠ Irreversible: a published version can be yanked, never deleted.

```powershell
Set-Location "$Root\rok-tools"; git switch main; git pull
cargo publish -p rok-tools --dry-run
cargo publish -p rok-tools-cli --dry-run       # fails until rok-tools is live; check only its own errors
cargo login                                    # once, with your crates.io token
cargo publish -p rok-tools
cargo publish -p rok-tools-cli
```

If `rok-tools-cli` names `rok-tools` by path only, an agent adds
`version = "0.1.0"` to that dependency first (`cargo publish` refuses a
path-only dependency).

**Verify:** `cargo search rok-tools --limit 2` lists both crates at
0.1.0.

---

## Step 6 — Create `rok-v2`, the `rok` command

**6a. Create and clone:**

```powershell
Set-Location $Root
gh repo create "$Gh/rok-v2" --public --clone --description "rok: one command for the rok toolchain"
Add-Content repos.toml 'rok-v2 = "https://github.com/ateeq1999/rok-v2.git"'
git commit -am "Add rok-v2"; git push
```

**6b. Agent task V1** (in `rok-v2`): the umbrella crate.

1. One crate, `name = "rok-cli"`, `version = "0.7.0"`, `[[bin]] name =
   "rok"`, MIT OR Apache-2.0, depending on `rok-data-cli = "0.6"` and
   `rok-tools-cli = "0.1"` from crates.io.
2. Routing: the linked CLIs' words (`init`, `gen`, `generate`, `migrate`,
   `db`, `server`, `schema`, `sqlc` → `rok_data_cli::run`; `tools` →
   `rok_tools_cli::run_from`) get their tokens verbatim; any other word
   `x` runs `rok-x` from the `PATH` with the remaining arguments and
   returns its exit code; nothing found is a usage error (exit 2) that
   names `rok-x`.
3. `rok --help` lists the linked commands (from each CLI's `command()`)
   and, under "Installed", every `rok-*` executable on the `PATH`.
4. Exit codes and `--json` per ADR 0002.
5. CI (`ci.yml`: fmt, clippy, test on Linux, macOS, Windows), and
   `release.yml` + `install.sh` + `install.ps1` moved from rok-data,
   building the `rok` binary from this repo and publishing `rok-cli`.
6. README, CHANGELOG `## [0.7.0]`, AGENTS.md, `docs/cli-reference.md`
   (the root grammar; each linked CLI's own reference stays in its repo).
7. Tests: routing to each linked CLI, the PATH fallback with a fake
   `rok-hello` on a temporary `PATH`, the help listing, exit codes.

**Accept:**

```powershell
Set-Location "$Root\rok-v2"
cargo fmt --all -- --check; cargo clippy --all-targets -- -D warnings; cargo test --all-targets
cargo run -q -- --help                         # lists init, gen, migrate, db, server, schema, sqlc, tools
cargo run -q -- tools --help                   # rok-tools' own help
cargo build --config ..\.cargo\patch-local.toml; cargo tree --config ..\.cargo\patch-local.toml -i rok-data-sqlc   # shows a path under rok-data
```

**6c. Release 0.7.0.** ⚠ Irreversible: from here `cargo install rok-cli`
installs the umbrella, not the old runner.

```powershell
gh secret set CARGO_REGISTRY_TOKEN --repo "$Gh/rok-v2"
git push -u origin HEAD; gh pr create --fill --base main; gh pr checks --watch; gh pr merge --squash --delete-branch
git switch main; git pull
git tag v0.7.0; git push origin v0.7.0
gh run watch (gh run list --workflow release.yml --limit 1 --json databaseId -q '.[0].databaseId')
```

**6d. Verify the install paths:**

```powershell
irm https://raw.githubusercontent.com/ateeq1999/rok-v2/main/install.ps1 | iex
rok --version                                  # rok 0.7.0
rok db check --help; rok tools --help
gh release view v0.7.0 --repo "$Gh/rok-v2"     # archives for every target + checksums
```

**6e. Agent task A3** (in `rok-data`): `release.yml` drops the binary
build, GitHub release and install-verification jobs (crates only from
now on); delete `install.sh` and `install.ps1`; README and
getting-started install `rok` from rok-v2; remove the commented
`rok-tools-cli` path dependency and the `rok_tools` cfg gate (`rok tools`
is rok-v2's now). **Accept:** step 3's four commands; `gh workflow view
release.yml` lists only the prepare and crates-io jobs.

---

## Step 7 — Nightly combined build in `rok-workspace`

```powershell
Set-Location $Root
New-Item -ItemType Directory -Force .github\workflows | Out-Null
@'
name: Combined build

# Builds and tests rok-v2 against every member repo's main, through
# .cargo/patch-local.toml: catches a break across repos before a release.
on:
  schedule:
    - cron: "0 3 * * *"
  workflow_dispatch:

jobs:
  combined:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Clone the member repos
        run: |
          grep -E '^[A-Za-z0-9_.-]+ *= *"' repos.toml | while IFS='=' read -r name url; do
            name=$(echo "$name" | xargs); url=$(echo "$url" | tr -d '" ')
            git clone --depth 1 "$url" "$name"
          done
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: rok-v2
      - name: Test rok-v2 against every member's main
        working-directory: rok-v2
        run: cargo test --all-targets --config ../.cargo/patch-local.toml
'@ | Set-Content -Encoding utf8 .github\workflows\combined.yml
git add .github; git commit -m "Nightly combined build"; git push
```

### Verify

```powershell
gh workflow run combined.yml --repo "$Gh/rok-workspace"
gh run watch --repo "$Gh/rok-workspace" (gh run list --repo "$Gh/rok-workspace" --limit 1 --json databaseId -q '.[0].databaseId')   # green
```

---

## Step 8 — Phase 2: split rok-data's CLI into `rok-db` and `rok-gen`

Start only after step 7 is green for a few nights.

**Agent task A4** (in `rok-data`, branch `toolchain-split/cli-split`):

1. `crates/rok-cli` becomes two crates: `crates/rok-db-cli` (crate
   `rok-db-cli`, binary `rok-db`: `server`, `db`, `migrate`, `schema`)
   and `crates/rok-gen-cli` (crate `rok-gen-cli`, binary `rok-gen`:
   `init`, `gen`, `generate`, `sqlc`), each exposing `command()` and
   `run_from(args)`, each with the tests of its commands (invariant 4
   becomes `CARGO_BIN_EXE_rok-gen` with `"sqlc"` prepended). Shared
   code (`context`, `config`, `output`, `keychain`) goes where its only
   user is, or into a small `rok-data-cli-support` library if both use
   it; split by responsibility (the 600-line rule).
2. `rok-data-cli` gets one last release whose README points to the two.
3. AGENTS.md (invariants 2–4), `docs/cli-reference.md` (split per CLI),
   CHANGELOG `[0.7.0]`.

**Accept:** step 3's four commands; `cargo run -p rok-db-cli -- --help`
and `cargo run -p rok-gen-cli -- --help` list exactly their commands.
Release rok-data 0.7.0 (tag as in 4d). ⚠ Irreversible: two new crate
names; crates.io limits new crates per account per hour — publish
fails with `429`, rerun the release job when it lifts.

**Agent task V2** (in `rok-v2`): link `rok-db-cli` and `rok-gen-cli`
instead of `rok-data-cli`; release `rok-cli` 0.8.0. **Accept:** step 6b's
commands, and `rok migrate status --help` / `rok sqlc --help` unchanged
from 0.7.0 apart from version strings.

---

## Rollback

| Undo | How |
| --- | --- |
| A deleted local repo | `git clone $Archive\<repo>.bundle $Root\<repo>` |
| An archived GitHub repo | `gh repo unarchive $Gh/<repo> --yes` |
| A merged rok-data / rok-templates / rok-v2 change | `gh pr create` reverting it; nothing outside the repo depends on `main` |
| `rok-workspace` | delete the folder's `.git` — the members are untouched |
| A crates.io release | `cargo yank --version X <crate>` stops new installs; it cannot be deleted |
