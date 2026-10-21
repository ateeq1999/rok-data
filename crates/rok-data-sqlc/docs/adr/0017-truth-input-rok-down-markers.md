# Truth inputs honor the whole-crate generator's `-- rok:down` convention

A truth file can describe more than its `CREATE TABLE`s say: an `ALTER TABLE books ADD COLUMN isbn TEXT`, and later statements like it, with the SQL that undoes each change beside it. The whole-crate generator (`rok-data-codegen`) already owns a vocabulary for that — a `-- rok:down` comment line sits in the whitespace between two statements, and the statement it introduces is the undo of the statement immediately before the marker; the generator keeps the undo for the migration pairs it writes and never applies it. Until now, the engine's schema parser had no such convention, so an undo carried in a truth file was a statement of its own as far as the schema was concerned, and applying it left the schema without the change the truth means to have.

**Decision.** On a truth input, the engine's schema parser honors that marker vocabulary: it scans the text for `-- rok:down` markers before the SQL parser reads the file, skips each marker together with the statement it introduces, and lets everything else through untouched — the file's line numbers survive, so its diagnostics still point where they did. The scan reads quotes, dollar quotes and comments the way the codegen's statement splitter does, so a `rok:down` line inside a string literal is that literal's text, in either tool.

The two engines refuse a misplaced marker at two points, and deliberately not the same way:

- a marker with no statement to undo — the file's first statement, or one whose previous statement was itself an undo — is refused by both, with the codegen's own wording ("`-- rok:down` must directly follow the statement it undoes");
- a marker whose undo never joined it — one a blank line parted from its statement, one met by an empty statement, and a trailing marker with nothing after it — is refused by the engine ("`-- rok:down` must be followed by the SQL that undoes the statement above it"), where the codegen either refuses the orphaned statement itself ("rok can't undo this statement by itself…") or, for a purely trailing marker, loses it silently. The engine cannot lose a marker: applying the statement below it is the wrong half of the pair, so the stronger refusal is the point.

The rule wording the refusals borrow is the codegen's rule told from the marker's side, so a marker a second engine misplaced is named the same way in both tools' outputs.

## Considered Options

- **Stay comment-blind: parse the undo as statement of its own.** Rejected: applied, an undo undoes the truth's own change — the schema a generated data layer comes from loses the column the truth means to have.
- **Structural `ALTER` support in both engines' diffs** (folding an `ALTER`ed column into the parsed `CREATE TABLE`s, so a later truth that folds it in stages no redundant drop-and-add pair). Deferred: it is the staging's remaining work, recorded in `docs/unified-pipeline-plan.md`'s staging Open note, and it stays open after this decision — this ADR fixes only what a truth input means, not what a staged migration folds.
- **A truth-file-only marker spelling** (`-- rok:down:` or a directives file beside the truth). Rejected: a second spelling that means a third, already established by the codegen's migrations, would split the vocabulary users type.
- **Silent acceptance of a misplaced marker**, as the codegen does with a trailing one. Rejected for the engine: silence here applies or loses data, never a road-bump the consumer can fix.

## Consequences

The pre-pass lives once, in `src/parser/postgres/undo.rs`, whose module docs state the convention, the scan and the refusals; `parse_statements` strips the markers before the SQL parser or the statement-recovery path sees the text. A truth file and the codegen's comment blocks now carry the same `-- rok:down` meaning, and the staged-pair behavior that reads them is unchanged: the ALTER still rides the staged pair as an `Extra` step with the undo the marker supplied, pending the staging's own vocabulary in the diff.
