//! One component name per `ToSchema` type: utoipa keeps a single schema
//! when two types share a name, so a hand-written DTO named like a
//! generated one would silently replace it in the OpenAPI document, and
//! every Client built from the document would come out wrong. A run reads
//! the App's source as it will be once the run is done (its planned files
//! over what is on disk, its stale files left out) and fails naming both
//! files of the first clash.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::bail;
use proc_macro2::{TokenStream, TokenTree};
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{Attribute, Generics, Ident, Token};

use crate::report::display_path;

/// Fails when two `ToSchema` types under `src` share a component name.
/// `planned` are the files the run writes, `stale` the ones it removes.
pub(super) fn check(
    src: &Path,
    planned: &[(PathBuf, String)],
    stale: &[PathBuf],
) -> anyhow::Result<()> {
    let mut sources: BTreeMap<PathBuf, Option<&str>> = BTreeMap::new();
    rust_files(src, &mut sources)?;
    let stale: HashSet<&PathBuf> = stale.iter().collect();
    sources.retain(|path, _| !stale.contains(path));
    for (path, text) in planned {
        if path.starts_with(src) && is_rust(path) {
            sources.insert(path.clone(), Some(text));
        }
    }

    let mut named: BTreeMap<String, (PathBuf, usize)> = BTreeMap::new();
    for (path, text) in &sources {
        let text = match text {
            Some(text) => (*text).to_owned(),
            None => std::fs::read_to_string(path)?,
        };
        // A file that does not parse is the compiler's to report.
        let Ok(file) = syn::parse_file(&text) else {
            continue;
        };
        let mut collector = Collector::default();
        collector.visit_file(&file);
        for (name, line) in collector.schemas {
            if let Some((first, first_line)) = named.get(&name) {
                bail!(
                    "two schemas are named `{name}`: {}:{first_line} and {}:{line}; the OpenAPI \
                     document keeps only one of them, so rename one of the types or give it \
                     `#[schema(as = ...)]`",
                    display_path(first),
                    display_path(path),
                );
            }
            named.insert(name, (path.clone(), line));
        }
    }
    Ok(())
}

fn is_rust(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension == "rs")
}

/// Every `.rs` file under `dir` (none when it doesn't exist), unread.
fn rust_files(dir: &Path, found: &mut BTreeMap<PathBuf, Option<&str>>) -> anyhow::Result<()> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() {
            rust_files(&path, found)?;
        } else if is_rust(&path) {
            found.insert(path, None);
        }
    }
    Ok(())
}

/// The component names one file's `ToSchema` types take, with their lines.
#[derive(Default)]
struct Collector {
    schemas: Vec<(String, usize)>,
}

impl Collector {
    fn item(&mut self, attrs: &[Attribute], ident: &Ident, generics: &Generics) {
        // A generic type's components are named per use; test code is not
        // in the document.
        if generics.type_params().next().is_some() || is_test(attrs) || !derives_schema(attrs) {
            return;
        }
        let name = attrs
            .iter()
            .filter(|attr| attr.path().is_ident("schema"))
            .find_map(|attr| {
                attr.meta
                    .require_list()
                    .ok()
                    .and_then(|list| schema_as(&list.tokens))
            })
            .unwrap_or_else(|| ident.to_string());
        self.schemas.push((name, ident.span().start().line));
    }
}

impl<'ast> Visit<'ast> for Collector {
    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        self.item(&item.attrs, &item.ident, &item.generics);
    }

    fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
        self.item(&item.attrs, &item.ident, &item.generics);
    }

    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        if !is_test(&item.attrs) {
            visit::visit_item_mod(self, item);
        }
    }

    fn visit_item_fn(&mut self, _: &'ast syn::ItemFn) {}
}

/// Whether `attrs` derive `ToSchema` (`utoipa::ToSchema` or imported).
fn derives_schema(attrs: &[Attribute]) -> bool {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("derive"))
        .filter_map(|attr| {
            attr.parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated)
                .ok()
        })
        .flatten()
        .any(|path| {
            path.segments
                .last()
                .is_some_and(|last| last.ident == "ToSchema")
        })
}

/// Whether `attrs` hold `#[cfg(test)]`.
fn is_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("cfg")
            && attr
                .parse_args::<Ident>()
                .is_ok_and(|ident| ident == "test")
    })
}

/// The name `as = path::Name` in a `#[schema(...)]` gives: utoipa joins
/// the path's segments with dots.
fn schema_as(tokens: &TokenStream) -> Option<String> {
    let mut tokens = tokens.clone().into_iter().peekable();
    while let Some(token) = tokens.next() {
        let TokenTree::Ident(ident) = &token else {
            continue;
        };
        let is_equals =
            |token: &TokenTree| matches!(token, TokenTree::Punct(punct) if punct.as_char() == '=');
        if ident != "as" || !tokens.peek().is_some_and(is_equals) {
            continue;
        }
        tokens.next();
        let value: TokenStream = tokens
            .by_ref()
            .take_while(|token| !matches!(token, TokenTree::Punct(punct) if punct.as_char() == ','))
            .collect();
        let path: syn::Path = syn::parse2(value).ok()?;
        let segments: Vec<String> = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect();
        return Some(segments.join("."));
    }
    None
}
