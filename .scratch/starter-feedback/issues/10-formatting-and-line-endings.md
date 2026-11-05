# 10: Templates formatted, LF enforced

Spec: `docs/specs/starter-feedback.md`

**What to build:** every template file is formatted with the pinned `dart format` and Prettier versions, the gates fail on an unformatted template file, and every Template ships `.gitattributes` with `* text=auto eol=lf`.

**Blocked by:** grpc-api-template/08 (the gRPC Template shipped)

**Status:** blocked

- [ ] Running the formatters on a fresh project changes nothing
- [ ] Each Template has the `.gitattributes`
