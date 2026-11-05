# 45: Calls to schema functions are typed

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** in query typing, a call to a function defined in the schema dir (`CREATE FUNCTION tz_today() RETURNS date`) is typed from its `RETURNS`, nullable by default (`Option<NaiveDate>`); `-- column: today chrono::NaiveDate` makes it not-null. `SELECT tz_today() AS today` generates a function.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] `SELECT tz_today() AS today` generates `Option<NaiveDate>`, and `NaiveDate` with the override (red first)
- [ ] An unknown function still fails under ticket 20's strict default
