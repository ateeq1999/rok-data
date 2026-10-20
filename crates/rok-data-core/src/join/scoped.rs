//! The scope of a joined model: the index markers and the tuple machinery.

use crate::{Column, Model, Projection};

use crate::join::scope::{Here, Last, There};

/// `Self` is the root or one of the joined models of the query scope `S`
/// (`(Root, Joins)`, with `Joins` = `()` or `(EarlierJoins, Model)`).
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not part of this query",
    label = "join `{Self}` first (`.join(…)` / `.left_join(…)`)"
)]
pub trait InScope<S, I> {}

// The root is found by walking back to the empty join list, so every model
// has exactly one proof (self-joins, which would make it ambiguous, are not
// supported).
impl<M> InScope<(M, ()), Here> for M {}
impl<M, Rest, N> InScope<(M, (Rest, N)), Last> for N {}
impl<X, M, Rest, N, I> InScope<(M, (Rest, N)), There<I>> for X where X: InScope<(M, Rest), I> {}

/// A value usable in a joined `select(…)`: a column or projection of some
/// model in the query.
pub trait ScopedItem {
    /// The model the item belongs to.
    type Model;
    #[doc(hidden)]
    fn into_projection(self) -> Projection<Self::Model>;
}

impl<X: Model> ScopedItem for Column<X> {
    type Model = X;
    fn into_projection(self) -> Projection<X> {
        self.into()
    }
}

impl<X> ScopedItem for Projection<X> {
    type Model = X;
    fn into_projection(self) -> Projection<X> {
        self
    }
}

/// The select list of a joined query: one [`ScopedItem`] or a tuple of up
/// to 8, each from a model in the query scope `S`.
pub trait ScopedProjections<S, I> {
    #[doc(hidden)]
    fn into_items<R>(self) -> Vec<Projection<R>>;
}

impl<S, I, A> ScopedProjections<S, (I,)> for A
where
    A: ScopedItem,
    A::Model: InScope<S, I>,
{
    fn into_items<R>(self) -> Vec<Projection<R>> {
        vec![self.into_projection().retag()]
    }
}

macro_rules! scoped_tuples {
    ($(($($t:ident $i:ident),+)),+ $(,)?) => {$(
        #[allow(non_snake_case)]
        impl<S, $($i,)+ $($t),+> ScopedProjections<S, ($($i,)+)> for ($($t,)+)
        where
            $($t: ScopedItem, $t::Model: InScope<S, $i>,)+
        {
            fn into_items<R>(self) -> Vec<Projection<R>> {
                let ($($t,)+) = self;
                vec![$($t.into_projection().retag()),+]
            }
        }
    )+};
}

scoped_tuples! {
    (A IA, B IB),
    (A IA, B IB, C IC),
    (A IA, B IB, C IC, D ID),
    (A IA, B IB, C IC, D ID, E IE),
    (A IA, B IB, C IC, D ID, E IE, F IF),
    (A IA, B IB, C IC, D ID, E IE, F IF, G IG),
    (A IA, B IB, C IC, D ID, E IE, F IF, G IG, H IH),
}
