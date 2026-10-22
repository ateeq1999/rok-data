//! The `Model` derive's conventions: the inference, the opt-outs, and the
//! contradictions that fail the derive.

use syn::DeriveInput;

use super::expand::expand;

/// The error the derive gives for `input`, or `None` when it expands.
fn the_error(input: DeriveInput) -> Option<String> {
    expand(input).err().map(|error| error.to_string())
}

/// The expanded tokens of `input`.
fn expanded(input: DeriveInput) -> String {
    expand(input).expect("the model expands").to_string()
}

/// Whether the expansion names `column` as the model's `CONSTANT`.
fn carries(expansion: &str, constant: &str, column: &str) -> bool {
    expansion.contains(&format!(
        "{constant} : :: core :: option :: Option < & 'static str > = \
         :: core :: option :: Option :: Some (\"{column}\")"
    ))
}

#[test]
fn a_marker_and_its_opt_out_contradict_each_other() {
    let error = the_error(syn::parse_quote! {
        #[rok(timestamps, no_timestamps)]
        struct Note {
            id: i64,
            created_at: String,
            updated_at: String,
        }
    })
    .expect("the contradiction fails the derive");
    assert!(error.contains("timestamps"), "{error}");
    assert!(error.contains("no_timestamps"), "{error}");

    let error = the_error(syn::parse_quote! {
        #[rok(soft_delete, no_soft_delete)]
        struct Note {
            id: i64,
            deleted_at: Option<String>,
        }
    })
    .expect("the contradiction fails the derive");
    assert!(error.contains("soft_delete"), "{error}");
    assert!(error.contains("no_soft_delete"), "{error}");
}

#[test]
fn an_opt_out_and_a_field_marker_contradict_each_other() {
    let error = the_error(syn::parse_quote! {
        #[rok(no_timestamps)]
        struct Note {
            id: i64,
            #[rok(created_at)]
            made_at: String,
            updated_at: String,
        }
    })
    .expect("the contradiction fails the derive");
    assert!(error.contains("no_timestamps"), "{error}");

    let error = the_error(syn::parse_quote! {
        #[rok(no_soft_delete)]
        struct Note {
            id: i64,
            #[rok(deleted_at)]
            removed_at: Option<String>,
        }
    })
    .expect("the contradiction fails the derive");
    assert!(error.contains("no_soft_delete"), "{error}");
}

#[test]
fn an_inferred_managed_column_cannot_be_generated() {
    let error = the_error(syn::parse_quote! {
        struct Note {
            id: i64,
            #[rok(generated)]
            deleted_at: Option<String>,
        }
    })
    .expect("the conflict fails the derive");
    assert!(error.contains("managed columns"), "{error}");
    assert!(error.contains("generated"), "{error}");

    let error = the_error(syn::parse_quote! {
        struct Note {
            id: i64,
            #[rok(generated)]
            created_at: String,
            updated_at: String,
        }
    })
    .expect("the conflict fails the derive");
    assert!(error.contains("managed columns"), "{error}");
}

#[test]
fn an_opt_out_leaves_the_inferred_column_alone() {
    // The opt-out turns the inference off, so the column is not managed and
    // may be `generated`.
    assert!(
        expand(syn::parse_quote! {
            #[rok(no_soft_delete)]
            struct Note {
                id: i64,
                #[rok(generated)]
                deleted_at: Option<String>,
            }
        })
        .is_ok()
    );
    assert!(
        expand(syn::parse_quote! {
            #[rok(no_timestamps)]
            struct Note {
                id: i64,
                #[rok(generated)]
                created_at: String,
                #[rok(generated)]
                updated_at: String,
            }
        })
        .is_ok()
    );
}

#[test]
fn the_field_names_infer_the_conventions() {
    let expanded = expanded(syn::parse_quote! {
        struct Note {
            id: i64,
            created_at: String,
            updated_at: String,
            deleted_at: Option<String>,
        }
    });
    assert!(
        carries(&expanded, "CREATED_AT_COLUMN", "created_at"),
        "{expanded}"
    );
    assert!(
        carries(&expanded, "UPDATED_AT_COLUMN", "updated_at"),
        "{expanded}"
    );
    assert!(
        carries(&expanded, "DELETED_AT_COLUMN", "deleted_at"),
        "{expanded}"
    );
}

#[test]
fn one_timestamp_column_alone_is_not_the_convention() {
    let expanded = expanded(syn::parse_quote! {
        struct Note {
            id: i64,
            created_at: String,
        }
    });
    assert!(
        !carries(&expanded, "CREATED_AT_COLUMN", "created_at"),
        "{expanded}"
    );
    assert!(
        !carries(&expanded, "UPDATED_AT_COLUMN", "updated_at"),
        "{expanded}"
    );
}

#[test]
fn the_opt_out_leaves_the_columns_ordinary() {
    let expanded = expanded(syn::parse_quote! {
        #[rok(no_timestamps, no_soft_delete)]
        struct Note {
            id: i64,
            created_at: String,
            updated_at: String,
            deleted_at: Option<String>,
        }
    });
    assert!(
        !carries(&expanded, "CREATED_AT_COLUMN", "created_at"),
        "{expanded}"
    );
    assert!(
        !carries(&expanded, "UPDATED_AT_COLUMN", "updated_at"),
        "{expanded}"
    );
    assert!(
        !carries(&expanded, "DELETED_AT_COLUMN", "deleted_at"),
        "{expanded}"
    );
}

#[test]
fn the_explicit_markers_still_work() {
    let expanded = expanded(syn::parse_quote! {
        #[rok(timestamps, soft_delete)]
        struct Note {
            id: i64,
            #[rok(created_at)]
            made_at: String,
            #[rok(updated_at)]
            touched_at: String,
            #[rok(deleted_at)]
            removed_at: Option<String>,
        }
    });
    assert!(
        carries(&expanded, "CREATED_AT_COLUMN", "made_at"),
        "{expanded}"
    );
    assert!(
        carries(&expanded, "UPDATED_AT_COLUMN", "touched_at"),
        "{expanded}"
    );
    assert!(
        carries(&expanded, "DELETED_AT_COLUMN", "removed_at"),
        "{expanded}"
    );
}
