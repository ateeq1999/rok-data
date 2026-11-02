# 04: the users screens and the signed-in shell

**Blocked by:** 02
**Status:** ready-for-agent

Spec: What ships (Users, The shell).

The users list (paged as the API pages it) and detail replace the starter's
notes; `AdaptiveScaffold` navigation, the Rok theme and a light/dark toggle.

Done when both screens are tested through their blocs, and render in light
and dark.

## Comments

Ticket 02 removed the starter's notes (the API serves none, so its provider
had to go) and left `UsersRepository` (`list(page)`, `get(id)`, in
`src/data/repositories/users_repository.rs`, registered in `src/app.rs`) for
these screens' blocs to read.
