//! `db::constraints`: the table the schema's `-- rok:constraint`
//! annotations generate.

use rok_data_sqlc_api_app::db::constraints::{ConstraintKind, lookup};

#[test]
fn an_annotated_constraint_has_its_field_and_message() {
    let folders = lookup("folders_owner_id_name_key").expect("annotated");
    assert_eq!(folders.table, Some("folders"));
    assert_eq!(folders.field, Some("name"));
    assert_eq!(folders.message, "You already have a folder by that name.");
    assert_eq!(folders.kind, ConstraintKind::Unique);

    let bookmarks = lookup("bookmarks_document_id_fkey").expect("annotated");
    assert_eq!(bookmarks.field, None);
    assert_eq!(bookmarks.kind, ConstraintKind::ForeignKey);

    assert_eq!(lookup("notes_pkey"), None);
}
