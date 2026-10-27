//! `docs/getting-started.md`'s samples, compiled.

use rok_data::prelude::*;

#[derive(Debug, Clone, Model)]
#[rok(has_many(notes = Note::AUTHOR_ID))]
struct User {
    #[rok(generated)]
    id: i64,
    name: String,
}

#[derive(Debug, Clone, Model)]
#[rok(scopes(popular = popular))]
struct Note {
    #[rok(generated)]
    id: i64,
    #[rok(belongs_to(author = User))]
    author_id: i64,
    text: String,
    views: i32,
}

fn popular(query: Select<Note>) -> Select<Note> {
    query.filter(Note::VIEWS.gt(100))
}

async fn tour(db: Db, note_id: i64, user_id: i64) -> rok_data::Result<()> {
    // reads: one row by key, the rest as a chain
    let note = Note::find_or_fail(&db, note_id).await?;
    let most_read = Note::filter(Note::VIEWS.gt(100))
        .order_by(Note::VIEWS.desc())
        .all(&db)
        .await?;

    // writes: the row-level verbs of `ModelWrite`
    let draft = Note {
        id: 0,
        author_id: user_id,
        text: "draft".into(),
        views: 0,
    };
    let stored = draft.insert(&db).await?;
    stored.delete(&db).await?;

    // relations: an accessor starts the related model's chain
    let author = note.author().first(&db).await?;
    let user = User::find_or_fail(&db, user_id).await?;
    let their_notes = user.notes().all(&db).await?;

    // named scopes: `#[rok(scopes(popular = popular))]` on the model
    let popular = Note::popular().all(&db).await?;
    Ok(())
}

async fn everyday(db: Db, user: User) -> rok_data::Result<()> {
    // conditions stay typed on the column; sugar for several columns
    let pattern = "%rust%";
    let found = Note::query()
        .filter_any([Note::TEXT], |column| column.ilike(pattern))
        .filter(Note::VIEWS.gt_column(Note::AUTHOR_ID))
        .get(&db)
        .await?;

    // filter by a relation, count it beside the rows, nest what it loads
    let writers = User::query().has(User::NOTES).all(&db).await?;
    let view = User::query()
        .with_count(User::NOTES)
        .with(User::NOTES.with(Note::AUTHOR))
        .all(&db)
        .await?;
    let how_many = view.count(User::NOTES, &user);
    let notes = view.get(User::NOTES, &user)?;

    // row verbs that write without reading first
    if let Some(note) = found.first() {
        let read_again = note.increment(&db, Note::VIEWS, 1).await?;
    }
    Ok(())
}
