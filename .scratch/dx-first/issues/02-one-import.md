# 02: One import brings the whole model surface

**What to build:** A user writes `use rok_data::prelude::*;` (and, for generated code, imports the table's module) and every verb works: `User::find`, `user.update`, `User::popular().active()`. Today `Book::find` needs `use rok_data::Model`, row writes need a second import `ModelWrite` (since the trait split), and chained scopes need `use …::models::UserScopes` in every calling module — the tests miss it because scopes and call sites share one module.

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] The prelude exports `Model`, `ModelWrite`, and whatever the scope chain needs
- [ ] Generated table modules re-export the derive's per-model scope trait, so importing the module is enough
- [ ] A facade test calls reads, row writes and chained scopes from a module **other than** the one declaring the model, with only the prelude imported
- [ ] Fixture and snapshot expectations refreshed through the sanctioned run
- [ ] getting-started shows the one import
