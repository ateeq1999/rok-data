//! The conventions a model carries: managed timestamps and soft deletes.
//!
//! The field names are the signal (workspace ADR 0015): a model with
//! `created_at` and `updated_at` fields manages those timestamps, a model
//! with a `deleted_at` field soft-deletes. The explicit markers
//! (`#[rok(timestamps)]`, `#[rok(soft_delete)]`) keep working and mean the
//! same thing; the opt-outs (`#[rok(no_timestamps)]`,
//! `#[rok(no_soft_delete)]`) turn the inference off. A model that carries
//! both a marker and its opt-out contradicts itself, and the derive says so
//! instead of picking one.

use syn::Ident;

use super::Field;

/// Which conventions a model carries.
pub(crate) struct Conventions {
    /// The model manages its timestamps: `created_at` and `updated_at` are
    /// set to `now()` on insert, and `updated_at` on every update.
    pub(crate) timestamps: bool,
    /// The model soft-deletes: `delete` sets `deleted_at`, and the queries
    /// skip the rows where it is set.
    pub(crate) soft_delete: bool,
}

/// The conventions a model carries, and the error a contradiction is.
pub(crate) fn decide(
    name: &Ident,
    columns: &[&Field],
    timestamps: bool,
    soft_delete: bool,
    no_timestamps: bool,
    no_soft_delete: bool,
) -> syn::Result<Conventions> {
    if timestamps && no_timestamps {
        return Err(syn::Error::new(
            name.span(),
            "`#[rok(timestamps)]` and `#[rok(no_timestamps)]` contradict each other; keep one",
        ));
    }
    if soft_delete && no_soft_delete {
        return Err(syn::Error::new(
            name.span(),
            "`#[rok(soft_delete)]` and `#[rok(no_soft_delete)]` contradict each other; keep one",
        ));
    }
    // A field that names itself as the managed column says the data layer
    // writes it; the opt-out says the data layer writes no such column.
    if no_timestamps && let Some(field) = columns.iter().find(|f| f.created_at || f.updated_at) {
        return Err(syn::Error::new(
            field.ident.span(),
            "`#[rok(no_timestamps)]` and the timestamp marker on this field contradict each other; keep one",
        ));
    }
    if no_soft_delete && let Some(field) = columns.iter().find(|f| f.deleted_at) {
        return Err(syn::Error::new(
            field.ident.span(),
            "`#[rok(no_soft_delete)]` and the `deleted_at` marker on this field contradict each other; keep one",
        ));
    }
    let named = |field: &str| columns.iter().any(|f| f.ident == field);
    Ok(Conventions {
        // Both timestamp fields are the signal: one of them alone is a
        // column the model happens to call `created_at`.
        timestamps: timestamps || (!no_timestamps && named("created_at") && named("updated_at")),
        soft_delete: soft_delete || (!no_soft_delete && named("deleted_at")),
    })
}
