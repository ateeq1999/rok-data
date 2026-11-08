//! The OpenAPI document's schema names: utoipa names each `ToSchema` type's
//! component after the type (or after its `#[schema(as = path::Name)]`, as
//! `path.Name`), and keeps one schema when two share a name. A hand-written
//! DTO named like a generated one would then silently replace it in the
//! contract, and the Clients would come out wrong.
//!
//! [`clashes`] reads a crate's `src/` lexically (each `.rs` file parsed
//! with `syn`, generated and hand-written alike) and finds the names two
//! types share. Test code (`#[cfg(test)]`) is left out; a file that does
//! not parse is skipped, since the compiler says what is wrong with it.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use proc_macro2::{TokenStream, TokenTree};
use syn::visit::Visit;
use syn::{Attribute, Ident};

/// One `ToSchema` type: where it is declared.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Declared {
    /// The file declaring it.
    pub path: PathBuf,
    /// The line of its `struct` or `enum` name.
    pub line: usize,
}

/// A component name two or more `ToSchema` types share.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clash {
    /// The component name (`UserResponse`, or `admin.UserResponse` for
    /// `#[schema(as = admin::UserResponse)]`).
    pub name: String,
    /// The types sharing it, by path and line.
    pub types: Vec<Declared>,
}

impl Clash {
    /// The clash as an error message, with `shown` rendering each path.
    pub fn message(&self, shown: impl Fn(&Path) -> String) -> String {
        let count = match self.types.len() {
            2 => "two".to_owned(),
            n => n.to_string(),
        };
        let mut message = format!(
            "{count} `ToSchema` types are named `{}` in the OpenAPI document, \
             and utoipa keeps only one of them:\n",
            self.name
        );
        for declared in &self.types {
            message.push_str(&format!("  {}:{}\n", shown(&declared.path), declared.line));
        }
        let leaf = self.name.rsplit('.').next().unwrap_or(&self.name);
        message.push_str(&format!(
            "rename one of them, or give it a name of its own with \
             `#[schema(as = some_module::{leaf})]`"
        ));
        message
    }
}

impl fmt::Display for Clash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message(|path| path.display().to_string()))
    }
}

/// The component names `ToSchema` types under `src` share, by name.
pub fn clashes(src: &Path) -> Result<Vec<Clash>> {
    let mut files = Vec::new();
    rust_files(src, &mut files)?;
    let mut by_name: BTreeMap<String, Vec<Declared>> = BTreeMap::new();
    for file in files {
        let text = fs::read_to_string(&file)
            .with_context(|| format!("failed to read {}", file.display()))?;
        let Ok(parsed) = syn::parse_file(&text) else {
            continue;
        };
        let mut collector = Collector::default();
        if !is_test(&parsed.attrs) {
            collector.visit_file(&parsed);
        }
        for (name, line) in collector.found {
            by_name.entry(name).or_default().push(Declared {
                path: file.clone(),
                line,
            });
        }
    }
    Ok(by_name
        .into_iter()
        .filter(|(_, types)| types.len() > 1)
        .map(|(name, types)| Clash { name, types })
        .collect())
}

/// Every `.rs` file under `dir`, sorted.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries = fs::read_dir(dir)
        .with_context(|| format!("failed to read {}", dir.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort();
    for path in entries {
        if path.is_dir() {
            rust_files(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

/// The `ToSchema` types of one file: component name and line.
#[derive(Default)]
struct Collector {
    found: Vec<(String, usize)>,
}

impl Collector {
    fn item(&mut self, attrs: &[Attribute], ident: &Ident) {
        if derives_to_schema(attrs) {
            let name = schema_as(attrs).unwrap_or_else(|| ident.to_string());
            self.found.push((name, ident.span().start().line));
        }
    }
}

impl<'ast> Visit<'ast> for Collector {
    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        self.item(&item.attrs, &item.ident);
    }

    fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
        self.item(&item.attrs, &item.ident);
    }

    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        if !is_test(&item.attrs) {
            syn::visit::visit_item_mod(self, item);
        }
    }

    fn visit_item_fn(&mut self, _: &'ast syn::ItemFn) {
        // A type declared in a function body is local to it.
    }
}

/// Whether `attrs` derive `ToSchema` (`utoipa::ToSchema` or imported).
fn derives_to_schema(attrs: &[Attribute]) -> bool {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("derive"))
        .filter_map(|attr| {
            attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
            )
            .ok()
        })
        .flatten()
        .any(|path| {
            path.segments
                .last()
                .is_some_and(|last| last.ident == "ToSchema")
        })
}

/// The name a `#[schema(as = path::Name<T>)]` gives: the path's segments
/// joined with `.`, without generics, as utoipa names it.
fn schema_as(attrs: &[Attribute]) -> Option<String> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("schema"))
        .find_map(|attr| match &attr.meta {
            syn::Meta::List(list) => as_value(list.tokens.clone()),
            _ => None,
        })
}

/// The `as = …` of a `schema(…)` argument list: the tokens after `as =`
/// up to the next top-level comma (one outside `<…>`).
fn as_value(tokens: TokenStream) -> Option<String> {
    let tokens: Vec<TokenTree> = tokens.into_iter().collect();
    let start = tokens.windows(2).position(|pair| {
        matches!(&pair[0], TokenTree::Ident(ident) if ident == "as")
            && matches!(&pair[1], TokenTree::Punct(punct) if punct.as_char() == '=')
    })?;
    let mut segments: Vec<String> = Vec::new();
    let mut depth = 0usize;
    for token in &tokens[start + 2..] {
        match token {
            TokenTree::Punct(punct) => match punct.as_char() {
                '<' => depth += 1,
                '>' => depth = depth.saturating_sub(1),
                ',' if depth == 0 => break,
                _ => {}
            },
            TokenTree::Ident(ident) if depth == 0 => segments.push(ident.to_string()),
            _ => {}
        }
    }
    (!segments.is_empty()).then(|| segments.join("."))
}

/// Whether `attrs` mark test code (`#[cfg(test)]`).
fn is_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("cfg")
            && attr
                .parse_args::<Ident>()
                .is_ok_and(|ident| ident == "test")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(source: &str) -> Vec<String> {
        let mut collector = Collector::default();
        collector.visit_file(&syn::parse_file(source).expect("parses"));
        collector.found.into_iter().map(|(name, _)| name).collect()
    }

    #[test]
    fn a_type_is_named_after_itself_or_its_schema_as() {
        let source = "#[derive(Debug, utoipa::ToSchema)]\nstruct A;\n\
                      #[derive(ToSchema)]\n#[schema(as = admin::B<T>, title = \"b\")]\nenum B {}\n\
                      #[derive(Debug)]\nstruct C;\n\
                      #[cfg(test)]\nmod tests { #[derive(ToSchema)] struct D; }\n\
                      mod nested { #[derive(ToSchema)] struct E; }\n";
        assert_eq!(names(source), ["A", "admin.B", "E"]);
    }
}
