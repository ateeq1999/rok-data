//! The `#[rok_data::test]` attribute macro.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{LitStr, Path, spanned::Spanned};

use crate::naming::crate_path;
pub(crate) fn expand_test(args: TokenStream2, item: syn::ItemFn) -> syn::Result<TokenStream2> {
    let mut migrations = Vec::new();
    let mut sql_files = Vec::new();
    let mut krate: Option<Path> = None;
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("migrations") {
            migrations.push(meta.value()?.parse::<LitStr>()?);
        } else if meta.path.is_ident("sql") {
            sql_files.push(meta.value()?.parse::<LitStr>()?);
        } else if meta.path.is_ident("crate") {
            krate = Some(meta.value()?.parse::<LitStr>()?.parse()?);
        } else {
            return Err(meta.error("expected `migrations`, `sql` or `crate`"));
        }
        Ok(())
    });
    syn::parse::Parser::parse2(parser, args)?;
    let krate = match krate {
        Some(path) => quote!(#path),
        None => crate_path(),
    };

    let syn::ItemFn {
        attrs,
        vis,
        sig,
        block,
        ..
    } = item;
    if sig.asyncness.is_none() {
        return Err(syn::Error::new(
            sig.fn_token.span(),
            "`#[rok_data::test]` functions must be `async`",
        ));
    }
    if sig.inputs.len() != 1 {
        return Err(syn::Error::new(
            sig.inputs.span(),
            "expected exactly one parameter, e.g. `db: Db`",
        ));
    }
    let syn::FnArg::Typed(param) = &sig.inputs[0] else {
        return Err(syn::Error::new(
            sig.inputs.span(),
            "expected a parameter like `db: Db`",
        ));
    };
    let (pat, ty) = (&param.pat, &param.ty);
    let name = &sig.ident;
    let output = &sig.output;
    let skip = match output {
        syn::ReturnType::Default => quote!(return),
        syn::ReturnType::Type(..) => quote!(return ::core::result::Result::Ok(())),
    };
    let migrate = migrations.iter().map(|dir| {
        quote! {
            __rok_test_db
                .migrate(::core::concat!(::core::env!("CARGO_MANIFEST_DIR"), "/", #dir))
                .await
                .expect(::core::concat!("failed to run migrations in ", #dir));
        }
    });
    let sql = sql_files.iter().map(|file| {
        quote! {
            __rok_test_db
                .db()
                .execute(::core::include_str!(::core::concat!(::core::env!("CARGO_MANIFEST_DIR"), "/", #file)))
                .await
                .expect(::core::concat!("failed to execute ", #file));
        }
    });
    Ok(quote! {
        #(#attrs)*
        #[::tokio::test]
        #vis async fn #name() #output {
            let __rok_test_db = match #krate::testing::TestDb::create().await {
                ::core::result::Result::Ok(::core::option::Option::Some(db)) => db,
                ::core::result::Result::Ok(::core::option::Option::None) => {
                    ::std::eprintln!("DATABASE_URL not set; skipping `{}`", ::core::stringify!(#name));
                    #skip;
                }
                ::core::result::Result::Err(e) => ::core::panic!("failed to create a test database: {e}"),
            };
            #(#migrate)*
            #(#sql)*
            let #pat: #ty = ::core::clone::Clone::clone(__rok_test_db.db());
            let __rok_result = async move #block.await;
            ::core::mem::drop(__rok_test_db);
            __rok_result
        }
    })
}
