# 90: Mail a feature words itself, with a Reply-To

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** in every API Template's `shared/mail` (actix, axum, the Monorepo API, and gRPC where it has one):

- **`Mail::reply_to: Option<String>`**, serde-defaulted so queued mail jobs still decode; `SmtpMailer` sets it as `Reply-To`, and the recording mailer keeps it.
- **`Mail::reply_to(self, address) -> Mail`**, so a call reads `templates.message(to, subject, &paragraphs).reply_to(business_email)`.
- **`Templates::message(to, subject, paragraphs)`**: text and HTML bodies worded like the account mails, each paragraph HTML-escaped.

Karibu Pass's Partner Enquiry mails the partnerships team, who reply to the business, not to `MAIL_FROM`; it added both by hand (ticket 27).

**Blocked by:** None (can start immediately)

**Status:** done (rok-templates PR #16)

- [x] A mail with `reply_to` reaches SMTP with a `Reply-To` header (transport test)
- [x] A job queued without `reply_to` still decodes
- [x] `Templates::message` escapes a paragraph's HTML
