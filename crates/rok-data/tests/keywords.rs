//! A relation named after a Rust keyword: the generated accessor is a raw
//! identifier (`r#type`), so the generated code compiles.

use rok_data::prelude::*;

/// The relation is named `type`: the accessor is `r#type`.
#[derive(Debug, Clone, PartialEq, Model)]
#[rok(has_many(type = Note::TYPE_ID))]
struct Post {
    #[rok(generated)]
    id: i64,
    title: String,
}

/// The relation's name is derived from the field: `type_id` gives `type`.
#[derive(Debug, Clone, PartialEq, Model)]
struct Note {
    #[rok(generated)]
    id: i64,
    #[rok(belongs_to = Post)]
    type_id: i64,
}

/// The accessors exist under their raw names, and the relation constants
/// keep their screaming-snake-case names.
#[test]
fn a_keyword_relation_gets_a_raw_accessor() {
    let accessor: fn(&Post) -> Select<Note> = Post::r#type;
    let _ = accessor;
    let accessor: fn(&Note) -> Select<Post> = Note::r#type;
    let _ = accessor;

    assert_eq!(Post::TYPE.foreign_key().name(), "type_id");
    assert_eq!(Note::TYPE.foreign_key().name(), "type_id");
}
