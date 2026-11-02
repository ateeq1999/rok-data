# 04: the users screens and the signed-in shell

**Blocked by:** 02
**Status:** done

Spec: What ships (Users, The shell).

The users list (paged as the API pages it) and detail replace the starter's
notes; `AdaptiveScaffold` navigation, the Rok theme and a light/dark toggle.

Done when both screens are tested through their blocs, and render in light
and dark.

- [x] The users list, paged as the API pages it, tested through its bloc
- [x] A user's detail screen, tested through its bloc
- [x] The starter's notes are gone, the users screens in their place
- [x] The signed-in shell is rok-ui's `AdaptiveScaffold` with navigation
- [x] The Rok theme, and a light/dark toggle in the shell
- [x] Both screens render in light and dark

## Comments

Ticket 02 removed the starter's notes (the API serves none, so its provider
had to go) and left `UsersRepository` (`list(page)`, `get(id)`, in
`src/data/repositories/users_repository.rs`, registered in `src/app.rs`) for
these screens' blocs to read.

**What landed.** `features/users`: `UsersBloc` (`PageRequested { page }`,
restartable; the last page's rows stay while the next loads; `page_count()`
from the API's `total` and `per_page`) and `UserBloc` (`Requested { id }`,
restartable; another user's detail never shows under a new id). The views:
`users_page` (a `Table` of the page with rok-ui's `Pagination`; a row opens
`/users/<id>`) and `user_page`. Both blocs are provided in `app.rs`.
`src/shell.rs` holds `AppShell` on `AdaptiveScaffold` (Home and Users) with
an app bar holding the light/dark toggle (`Theme::toggle_mode`) and sign-out
(the `AuthBloc`'s `SignOutRequested`). It also holds `install_theme`, which
`main.rs` calls: the Rok preset, in the system's appearance. Routes: the
signed-in pages sit under the pathless `_app/` layout (`/`, `/users`,
`/users/:id`), `/auth` stays outside the shell, and `__root.rs` keeps only
the session guard.

**Gates.** `tests/features/users/` has 7 bloc tests on a fake repository.
`tests/shell/` has 3 GPUI tests that render the App itself against a mock
API. Two of them open the list, click a row and get the detail, one in Rok
light and one in Rok dark. The third checks that the toggle flips the mode
and keeps the preset.

**With 03 merged.** `AppShell` sends a signed-out session (a start with no
kept token, or one just signed out) to sign-in and renders nothing, since
`SessionGuard` only reacts to changes. `tests/shell/` builds the App as
`main.rs` does (`Api::kept`), on a `MemoryTokenStore`, never the keychain.
Two more tests cover it: a signed-out start lands on `/auth`, and the
shell's sign-out lands on `/auth` and clears the kept token (5 tests).
