# The upstream starters' READMEs, for their archive

Workspace ADR 0027: rok-templates is the only source of the monorepo's web
and mobile Apps. Its two upstream repositories are archived with a pointer
here, so nobody edits a second copy `template_drift` cannot see. Ticket 09
prepares the text; the owner does the rest:

1. Replace each repository's `README.md` with the text below (one commit,
   on its default branch).
2. Archive the repository (Settings → General → Danger Zone → Archive this
   repository).

Neither step has been done from this repository: both are owner actions on
GitHub.

Imported at:

| Repository | Imported commit | Now |
| --- | --- | --- |
| `ateeq1999/tanstack-starter-axum` | not recorded by ticket 03 (rok-data commit `d6697a8` imported it) | `crates/rok-templates/templates/monorepo/apps/web/` |
| `ateeq1999/Nestjs-Better-Auth-Flutter-Starter` | `13e21398dc53bd09d0ad08310778177412509e43` | `crates/rok-templates/templates/monorepo/apps/mobile/` |

---

## `ateeq1999/tanstack-starter-axum` — README.md

```markdown
# tanstack-starter-axum (archived)

This starter moved. It is now the **web App** of rok-data's monorepo
template, and that copy is the only one kept up to date:

- Source: [`crates/rok-templates/templates/monorepo/apps/web/`](https://github.com/ateeq1999/rok-data/tree/main/crates/rok-templates/templates/monorepo/apps/web)
  in [ateeq1999/rok-data](https://github.com/ateeq1999/rok-data) (the
  `rok-templates` crate).
- Guide: [The monorepo template](https://github.com/ateeq1999/rok-data/blob/main/crates/rok-templates/docs/guide/monorepo.md).

Start a project with it:

    cargo install rok-data-cli
    rok init my-app --template monorepo --org com.example

That writes the web App beside the axum API it talks to (`apps/api`), a
desktop and a mobile App, and the clients generated from the API's
OpenAPI document. The web App no longer has its own HTTP layer: it calls
the API through the generated TypeScript client (`clients/ts`), which
`rok gen` keeps in step with the API.

This repository is archived and read-only. Open issues and pull requests
against [ateeq1999/rok-data](https://github.com/ateeq1999/rok-data).
```

---

## `ateeq1999/Nestjs-Better-Auth-Flutter-Starter` — README.md

```markdown
# Nestjs-Better-Auth-Flutter-Starter (archived)

This starter moved. It is now the **mobile App** of rok-data's monorepo
template, and that copy is the only one kept up to date:

- Source: [`crates/rok-templates/templates/monorepo/apps/mobile/`](https://github.com/ateeq1999/rok-data/tree/main/crates/rok-templates/templates/monorepo/apps/mobile)
  in [ateeq1999/rok-data](https://github.com/ateeq1999/rok-data) (the
  `rok-templates` crate).
- Guide: [The monorepo template](https://github.com/ateeq1999/rok-data/blob/main/crates/rok-templates/docs/guide/monorepo.md).

Start a project with it:

    cargo install rok-data-cli
    rok init my-app --template monorepo --org com.example

`--org` and the project's name set the package name, the Android
`applicationId`, the iOS bundle id, the deep-link scheme and the display
name.

What changed on the way: the App talks to rok-data's axum API (`apps/api`)
instead of a NestJS / Better Auth backend, through the generated Dart
client (`clients/dart`), which `rok gen` keeps in step with the API. The
features that API does not serve yet (organizations, magic-link sign-in,
the admin panel, deleting the account, OAuth sign-in) are not in the App.
Push notifications (FCM) are a written recipe:
[Push notifications](https://github.com/ateeq1999/rok-data/blob/main/crates/rok-templates/docs/guide/recipes/push-notifications.md).
The Firebase configuration this repository held is not carried over.

This repository is archived and read-only. Open issues and pull requests
against [ateeq1999/rok-data](https://github.com/ateeq1999/rok-data).
```
