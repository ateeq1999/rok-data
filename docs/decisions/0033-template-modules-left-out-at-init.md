# ADR 0033: A Template's Modules can be left out at init

Date: 2026-11-03 · Status: accepted (amends ADR 0027 and ADR 0029)

## Context

A project scaffolded from the Monorepo template (Karibu Pass, at
`rok-cli` 0.7.0) spent real time unwiring parts of the template it did
not need: API keys, the media library, the audit log, admin users, TOTP
and seed-colour, radius and density theming. Each has its own routes,
data and screens across the Apps, so removing one by hand meant editing
API, web and mobile code and the template's own tests, and every rok
upgrade re-opened those edits. These parts are not Capabilities:
a Capability (mail, storage, jobs) has no routes or screens and is inert
until configured, so leaving one in costs nothing.

## Decision

- **Module** is the name for a ready-made product surface of a Template
  (its routes, data and screens across the Apps) that can be left out
  at init. Capabilities are not Modules and stay in every project.
- **`rok init … --without <MODULE,…>`** scaffolds the Template without
  those Modules. The rest of the project compiles and passes its tests:
  nothing outside a Module refers to it except at the wiring points the
  catalog names for it.
- **The catalog declares each Module**: its name, its files in each App,
  and its wiring points. The Template's copy is a single source; leaving
  a Module out drops its files and its lines at the wiring points, it is
  not a second copy of the Template.
- **The gates build two combinations** of every Template that has
  Modules: the default (all in) and all out. A combination in between
  is only as good as those two, so a Module that leans on another must
  say so in the catalog, and `--without` refuses to leave out a Module
  another kept one needs.
- **A Blueprint records the choice**: with the Template, its
  Integrations and its init options, a Blueprint holds the API framework
  (`-w`) and the Modules left out, so projects created from it come out
  alike.

## Considered Options

- **Presets** (`--preset minimal`): fewer choices to make, but every
  preset is a combination someone must keep compiling, and a project
  that needs one Module of a preset gets none. Presets can come later as
  names for `--without` lists.
- **A removal guide**: no catalog work, but it is the hand-editing this
  decision exists to remove, repeated by every project and broken by
  every upgrade.

## Consequences

- Each Module's code has to stay inside its own files and its declared
  wiring points; a reference from outside them is a gate failure in the
  all-out build, not a review comment.
- `--without` names are part of the CLI surface and the Blueprint
  format, so renaming a Module is a breaking change.
