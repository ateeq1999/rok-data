// Scaffolded once by rok-data-sqlc; this file is yours.

//! Read models a query decodes into directly (`-- returns:` with a path):
//! no row struct is generated for them, and nothing is copied field by
//! field.

/// A project's id and name, for a picker.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct ProjectName {
    /// The project's id.
    pub id: uuid::Uuid,
    /// Its name.
    pub name: String,
}
