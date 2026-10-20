//! Joins: fetching several models in one query.

mod condition;
mod joined;
mod scoped;

pub use condition::{IntoJoin, JoinCondition, JoinOn};
pub use joined::{Joined, JoinedModels};
pub use scoped::{InScope, ScopedItem, ScopedProjections};

/// Index markers proving a model is part of a joined query. Inferred by the
/// compiler; you never write them.
#[doc(hidden)]
pub mod scope {
    use std::marker::PhantomData;

    /// The root model.
    #[derive(Debug)]
    pub struct Here;
    /// The most recently joined model.
    #[derive(Debug)]
    pub struct Last;
    /// A model joined earlier.
    #[derive(Debug)]
    pub struct There<I>(PhantomData<I>);
}
