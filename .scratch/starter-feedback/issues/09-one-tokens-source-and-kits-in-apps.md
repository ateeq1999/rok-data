# 09: One `tokens.json`, and each kit in its App

Spec: `docs/specs/starter-feedback.md`

**What to build:** the Monorepo template ships one `tokens.json`; its web, mobile and desktop tokens are generated from it by `rok tools tokens`, its CI runs `rok tools tokens --check`, and each UI kit moves into its App's own source instead of shipping as a package.

**Blocked by:** 08

**Status:** blocked

- [ ] The three Apps' tokens come from `tokens.json`; CI fails on drift
- [ ] No kit is a package; the web, mobile and desktop gates pass
