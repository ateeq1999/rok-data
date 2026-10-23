//! The scopes the generated models declare (`[generate.scopes]`): the
//! configuration names these functions by path, and the derive checks their
//! shape when the models compile (spec D7).

#![allow(dead_code)]

pub mod suppliers {
    //! The named scopes of `db::suppliers`.

    use crate::db::suppliers::models::Supplier;
    use rok_data::Select;

    /// The `active` scope: the suppliers still taking orders.
    #[must_use]
    pub fn active(query: &Select<Supplier>) -> Select<Supplier> {
        query.clone().filter(Supplier::IS_ACTIVE.eq(true))
    }
}
