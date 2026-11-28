//! How an actix-web project's features mount their routes, read with `syn`,
//! and the two warnings about it:
//!
//! - **A covering scope:** a `web::scope` answers `404` for every path under
//!   its prefix it doesn't route itself, so a scope in one feature hides
//!   another feature's routes under that prefix when it is mounted first.
//! - **A fixed path after a parameterised one that matches it:** actix-web
//!   picks the first resource whose path matches, and that one answers
//!   `405` for a method it doesn't route, so `POST /checkout/tickets` never
//!   reaches its handler behind `GET /checkout/{id}`.
//!
//! The mounts read are `web::scope("...")` and `web::resource("...")` (or
//! any function named `scope` or `resource`, such as the generated
//! `shared::validation::resource`), and `.route("...", ...)` on a scope or a
//! `ServiceConfig`. A scope's routes and services are under its prefix.
//! Test code is exempt.

use syn::visit::{self, Visit};
use syn::{Expr, ExprCall, ExprLit, ItemFn, ItemMod, Lit};

use super::caller::is_test;
use super::{Rule, Violation};

/// A path a feature mounts, or a scope's prefix: the full path, and the
/// line of its literal.
#[derive(Debug, Clone)]
pub(super) struct Mount {
    pub(super) path: String,
    pub(super) line: usize,
}

/// What one file mounts, in the order it mounts it.
#[derive(Debug, Default)]
pub(super) struct Mounts {
    /// Every `web::scope`'s full prefix.
    pub(super) scopes: Vec<Mount>,
    /// Every resource's and route's full path.
    pub(super) paths: Vec<Mount>,
}

/// Reads what `file` mounts.
fn read(file: &syn::File) -> Mounts {
    let mut collector = Collector::default();
    if !is_test(&file.attrs) {
        collector.visit_file(file);
    }
    collector.mounts
}

/// The warnings of a project's feature files, each given as its feature,
/// its path as shown, and its parsed source.
pub(super) fn violations(files: &[(String, String, &syn::File)]) -> Vec<Violation> {
    let read: Vec<(&str, &str, Mounts)> = files
        .iter()
        .map(|(feature, shown, file)| (feature.as_str(), shown.as_str(), read(file)))
        .collect();
    let mut found = Vec::new();
    for (feature, shown, mounts) in &read {
        let mut report = |line: usize, rule: Rule, message: String| {
            found.push(Violation {
                path: (*shown).to_owned(),
                line,
                rule,
                message,
            });
        };
        route_order(mounts, &mut report);
        let others: Vec<(&str, &Mounts)> = read
            .iter()
            .filter(|(other, _, _)| other != feature)
            .map(|(other, _, mounts)| (*other, mounts))
            .collect();
        covering_scopes(mounts, &others, &mut report);
    }
    found
}

/// Reports each path of one file mounted after a parameterised one that
/// matches it.
fn route_order(mounts: &Mounts, report: &mut impl FnMut(usize, Rule, String)) {
    for (index, later) in mounts.paths.iter().enumerate() {
        if let Some(earlier) = mounts.paths[..index]
            .iter()
            .find(|earlier| shadows(&earlier.path, &later.path))
        {
            report(
                later.line,
                Rule::RouteOrder,
                format!(
                    "`{}` is mounted after `{}` (line {}), which matches it and answers 405 \
                     for a method it doesn't route: mount the fixed path first",
                    later.path, earlier.path, earlier.line
                ),
            );
        }
    }
}

/// Reports each scope of one feature's file whose prefix covers a path of
/// another feature, `others` holding each other feature's name and mounts.
fn covering_scopes(
    mounts: &Mounts,
    others: &[(&str, &Mounts)],
    report: &mut impl FnMut(usize, Rule, String),
) {
    for scope in &mounts.scopes {
        for (feature, theirs) in others {
            for path in theirs
                .paths
                .iter()
                .filter(|path| covers(&scope.path, &path.path))
            {
                report(
                    scope.line,
                    Rule::CoveringScope,
                    format!(
                        "`web::scope(\"{}\")` covers feature `{feature}`'s `{}`, which then \
                         answers 404 when this scope is mounted first: mount `web::resource`s, \
                         not a scope",
                        scope.path, path.path
                    ),
                );
            }
        }
    }
}

/// A path's segments, without the empty ones.
fn segments(path: &str) -> Vec<&str> {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn is_parameter(segment: &str) -> bool {
    segment.starts_with('{')
}

/// Whether a request for the fixed parts of `later` would reach `earlier`
/// first: as many segments, each of `earlier`'s a parameter or the same,
/// and at least one a parameter where `later`'s is fixed.
fn shadows(earlier: &str, later: &str) -> bool {
    let (earlier, later) = (segments(earlier), segments(later));
    earlier.len() == later.len()
        && earlier
            .iter()
            .zip(&later)
            .all(|(e, l)| is_parameter(e) || e == l)
        && earlier
            .iter()
            .zip(&later)
            .any(|(e, l)| is_parameter(e) && !is_parameter(l))
}

/// Whether the scope `prefix` matches requests for `path`.
fn covers(prefix: &str, path: &str) -> bool {
    let (prefix, path) = (segments(prefix), segments(path));
    prefix.len() <= path.len()
        && prefix
            .iter()
            .zip(&path)
            .all(|(p, segment)| is_parameter(p) || p == segment)
}

#[derive(Default)]
struct Collector {
    mounts: Mounts,
    /// The prefix of the scope being read, if any.
    prefix: String,
}

impl Collector {
    /// Reads a call chain: `web::scope("/x").route(...).service(...)`,
    /// `cfg.service(...)`, `web::resource("/x").route(...)`.
    fn chain(&mut self, expr: &Expr) {
        let mut calls = Vec::new();
        let mut root = expr;
        while let Expr::MethodCall(call) = root {
            calls.push(call);
            root = &call.receiver;
        }
        calls.reverse();
        let outer = self.prefix.clone();
        let mut base = outer.clone();
        match root {
            Expr::Call(call) => match mount(call) {
                Some((Kind::Scope, path, line)) => {
                    base = format!("{outer}{path}");
                    self.mounts.scopes.push(Mount {
                        path: base.clone(),
                        line,
                    });
                }
                Some((Kind::Resource, path, line)) => self.mounts.paths.push(Mount {
                    path: format!("{outer}{path}"),
                    line,
                }),
                None => visit::visit_expr_call(self, call),
            },
            other => visit::visit_expr(self, other),
        }
        self.prefix = base;
        for call in calls {
            let mut args = call.args.iter();
            if call.method == "route"
                && call.args.len() == 2
                && let Some((path, line)) = call.args.first().and_then(literal)
            {
                self.mounts.paths.push(Mount {
                    path: format!("{}{path}", self.prefix),
                    line,
                });
                args.next();
            }
            for arg in args {
                self.visit_expr(arg);
            }
        }
        self.prefix = outer;
    }
}

impl<'ast> Visit<'ast> for Collector {
    fn visit_expr(&mut self, expr: &'ast Expr) {
        match expr {
            Expr::MethodCall(_) | Expr::Call(_) => self.chain(expr),
            _ => visit::visit_expr(self, expr),
        }
    }

    fn visit_item_fn(&mut self, item: &'ast ItemFn) {
        if !is_test(&item.attrs) {
            visit::visit_item_fn(self, item);
        }
    }

    fn visit_item_mod(&mut self, item: &'ast ItemMod) {
        if !is_test(&item.attrs) {
            visit::visit_item_mod(self, item);
        }
    }
}

enum Kind {
    Scope,
    Resource,
}

/// `scope("/x")` or `resource("/x")`, by any path: its kind, path and the
/// literal's line.
fn mount(call: &ExprCall) -> Option<(Kind, String, usize)> {
    let Expr::Path(func) = call.func.as_ref() else {
        return None;
    };
    let kind = match func.path.segments.last()?.ident.to_string().as_str() {
        "scope" => Kind::Scope,
        "resource" => Kind::Resource,
        _ => return None,
    };
    let (path, line) = literal(call.args.first()?)?;
    Some((kind, path, line))
}

/// A string literal's value and line.
fn literal(expr: &Expr) -> Option<(String, usize)> {
    match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Str(text),
            ..
        }) => Some((text.value(), text.span().start().line)),
        _ => None,
    }
}
