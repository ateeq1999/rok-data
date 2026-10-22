//! Named scopes: the trait the chain's scope methods hang off, and the
//! macro that declares a model's scopes.

use crate::Model;
use crate::query::Select;

/// The machinery of a **named scope**: a function from a query chain to a
/// query chain, `fn(&Select<M>) -> Select<M>`.
///
/// A model declares its scopes by path
/// (`#[rok(scopes(popular = path::to::popular))]`), and the derive gives
/// the model an entry (`User::popular()`) and the chain a method
/// (`User::popular().active()`). This trait is what the chain's method
/// calls: it applies a scope function to a chain.
///
/// ```ignore
/// fn popular(query: &Select<User>) -> Select<User> {
///     query.clone().filter(User::VIEWS.gt(100))
/// }
///
/// let chain = rok_data::Scope::scope(User::query(), popular);
/// ```
///
/// The chain's own [`scope`](crate::Select::scope) method takes a closure
/// and shadows this one in method syntax, so this trait's method is
/// reached as `Scope::scope(chain, popular)` — the way the generated code
/// reaches it.
pub trait Scope<M: Model>: Sized {
    /// Apply the scope function `apply` to this chain.
    fn scope(self, apply: fn(&Select<M>) -> Select<M>) -> Select<M>;
}

impl<M: Model> Scope<M> for Select<M> {
    fn scope(self, apply: fn(&Select<M>) -> Select<M>) -> Select<M> {
        apply(&self)
    }
}

/// Declare a model's named scopes.
///
/// The macro writes the trait the chain's scope methods hang off: one
/// method per scope, implemented for the model's chain. The
/// `#[derive(Model)]` macro calls it for you — a model that declares
/// `#[rok(scopes(popular = path::to::popular))]` gets a `UserScopes`
/// trait with a `popular` method — so hand-writing the macro is only for
/// a model that declares scopes without the derive.
///
/// ```ignore
/// rok_data::declare_scopes! {
///     /// The named scopes of `User`.
///     pub trait UserScopes for User {
///         popular => popular,
///         active => active,
///     }
/// }
/// ```
///
/// The scope functions are named by path and have the shape
/// `fn(&Select<M>) -> Select<M>`; the macro checks that they do.
#[macro_export]
macro_rules! declare_scopes {
    (
        $(#[$meta:meta])*
        $vis:vis trait $trait_name:ident for $model:ty {
            $($scope:ident => $path:path),* $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis trait $trait_name: $crate::Scope<$model> {
            $(
                fn $scope(self) -> $crate::Select<$model>;
            )*
        }

        impl $trait_name for $crate::Select<$model> {
            $(
                fn $scope(self) -> $crate::Select<$model> {
                    $crate::Scope::scope(self, $path)
                }
            )*
        }
    };
}
