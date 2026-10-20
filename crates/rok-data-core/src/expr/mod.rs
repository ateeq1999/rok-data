//! The expression layer: columns, expressions, conditions, projections, orders.

mod column;
mod cond;
mod order;
mod projection;
mod value;

pub use column::Column;
pub(crate) use cond::Cond;
pub(crate) use cond::SubKind;
pub(crate) use order::OrderTarget;
pub use order::{Direction, Order};
pub use projection::{IntoProjections, Projection};
pub use value::Expr;
pub(crate) use value::{Render, Term};

macro_rules! impl_into_projections_for_tuples {
    ($(($($t:ident),+)),+ $(,)?) => {$(
        #[allow(non_snake_case)]
        impl<M, $($t: Into<Projection<M>>),+> IntoProjections<M> for ($($t,)+) {
            fn into_projections(self) -> Vec<Projection<M>> {
                let ($($t,)+) = self;
                vec![$($t.into()),+]
            }
        }
    )+};
}

impl_into_projections_for_tuples! {
    (A),
    (A, B),
    (A, B, C),
    (A, B, C, D),
    (A, B, C, D, E),
    (A, B, C, D, E, F),
    (A, B, C, D, E, F, G),
    (A, B, C, D, E, F, G, H),
    (A, B, C, D, E, F, G, H, I),
    (A, B, C, D, E, F, G, H, I, J),
    (A, B, C, D, E, F, G, H, I, J, K),
    (A, B, C, D, E, F, G, H, I, J, K, L),
}

#[cfg(test)]
mod tests;
