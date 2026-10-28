//! Emitting Rust source laid out as rustfmt lays it out: `use` lists, docs,
//! function heads, method chains, and derives.

use super::sql;
use crate::Derive;
use crate::schema::mentions_generated_type;

/// rustfmt's default `max_width`; generated code is laid out to be
/// rustfmt-stable so regenerated files diff cleanly.
pub(super) const MAX_WIDTH: usize = 100;

thread_local! {
    /// The columns the caller of [`indented`] adds in front of every line.
    static INDENT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The width a line rendered at column 0 may take: [`MAX_WIDTH`], less the
/// indent the caller adds afterwards (inside [`indented`]). Rendering at
/// `MAX_WIDTH - indent` and then indenting breaks exactly the lines rustfmt
/// breaks at `MAX_WIDTH`; the chain and call widths measure content, not
/// indent, so they stay as they are.
pub(super) fn max_width() -> usize {
    MAX_WIDTH - INDENT.with(std::cell::Cell::get)
}

/// Runs `render` laid out for lines its caller indents by `indent` columns,
/// such as items placed in an `impl` block afterwards.
pub(super) fn indented<T>(indent: usize, render: impl FnOnce() -> T) -> T {
    let before = INDENT.with(|cell| cell.replace(indent));
    let rendered = render();
    INDENT.with(|cell| cell.set(before));
    rendered
}

/// rustfmt's default `chain_width`: a method chain longer than this goes one
/// call per line.
pub(super) const CHAIN_WIDTH: usize = 60;

/// rustfmt's default `fn_call_width`: call arguments longer than this go one
/// per line.
pub(super) const FN_CALL_WIDTH: usize = 60;

/// The head of a chain on `model` written at `indent` after `lead`:
/// `Book::filter(…)` when the first call is a filter (taken out of
/// `calls`), else `Book::query()` — the model's own entry, the way a
/// hand-written call site reads. The filter folds in only while the head
/// stays on one line with a column to spare and its argument within
/// `fn_call_width`; past that rustfmt would break the head's arguments,
/// and `Book::query()` keeps the layout simple.
pub fn model_chain(model: &str, calls: &mut Vec<String>, indent: usize, lead: &str) -> String {
    if let Some(argument) = calls
        .first()
        .and_then(|first| first.strip_prefix(".filter("))
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let head = format!("{model}::filter({argument})");
        if argument.len() <= FN_CALL_WIDTH && indent + lead.len() + head.len() < max_width() {
            calls.remove(0);
            return head;
        }
    }
    format!("{model}::query()")
}

/// Appends the method chain `{head}{calls}` at `indent`, after `lead` (such
/// as `"&& "`): on one line when the chain fits [`CHAIN_WIDTH`], otherwise
/// with `head` on the first line and each call on its own line below,
/// indented once more, as rustfmt splits it.
pub fn push_chain(output: &mut String, indent: usize, lead: &str, head: &str, calls: &[String]) {
    let pad = " ".repeat(indent);
    let chain = format!("{head}{}", calls.concat());
    if chain.len() <= CHAIN_WIDTH {
        output.push_str(&format!("{pad}{lead}{chain}\n"));
        return;
    }
    output.push_str(&format!("{pad}{lead}{head}\n"));
    for call in calls {
        output.push_str(&format!("{pad}    {call}\n"));
    }
}

/// A chain framed the way rustfmt frames a statement: a two-link chain
/// (`head` and one call) fits the line width alone; three or more links
/// must also fit [`CHAIN_WIDTH`]. Otherwise the head goes on the first
/// line, one call per line, with `trail` (such as `;`) on the last call.
pub fn push_wide_chain(
    output: &mut String,
    indent: usize,
    lead: &str,
    head: &str,
    calls: &[String],
    trail: &str,
) {
    let pad = " ".repeat(indent);
    let chain = format!("{head}{}", calls.concat());
    let line_fits = pad.len() + lead.len() + chain.len() + trail.len() <= max_width();
    // The chain's width budget covers the whole expression, `.await` and a
    // trailing `?` included; the statement's lead and `;` do not count.
    // A trailing `?` costs two columns (probed: a 60-column chain stays on
    // its line, the same chain at 59 columns plus `?` breaks).
    let wide = calls.len() <= 1;
    let try_width = 2 * usize::from(trail.starts_with('?'));
    let one_line = line_fits && (wide || chain.len() + try_width <= CHAIN_WIDTH);
    if one_line {
        output.push_str(&format!("{pad}{lead}{chain}{trail}\n"));
        return;
    }
    // Overfull two-linked statement: rustfmt breaks after the assignment
    // and keeps the chain intact on the continuation line.
    if wide && lead.ends_with("= ") {
        let continuation = indent + 4;
        if continuation + chain.len() + trail.len() <= max_width() {
            output.push_str(&format!("{pad}{}\n", lead.trim_end()));
            output.push_str(&format!("{}{chain}{trail}\n", " ".repeat(continuation)));
            return;
        }
    }
    // Vertical: one call per line. A call is framed with its arguments one
    // per line when the arguments, as written inside its parentheses,
    // exceed the chain width; a framed argument that would still overflow
    // breaks as its own chain (head, then one link per line).
    let mut link_pad = indent + 4;
    if lead.ends_with("= ") && pad.len() + lead.len() + head.len() >= max_width() {
        // The head leaves no column to spare after `let … =`: rustfmt
        // moves the whole chain below the `=`, one level in (probed).
        output.push_str(&format!("{pad}{}\n", lead.trim_end()));
        output.push_str(&format!("{}{head}\n", " ".repeat(indent + 4)));
        link_pad += 4;
    } else {
        output.push_str(&format!("{pad}{lead}{head}\n"));
    }
    for (index, call) in calls.iter().enumerate() {
        let suffix = if index + 1 == calls.len() { trail } else { "" };
        push_chain_call(output, link_pad, call, suffix);
    }
}

/// `Ok(<chain>)` as a function's tail, laid out as rustfmt lays it out: on
/// one line when the chain fits there; else the head beside `Ok(` and one
/// call per line below it, while the head fits the function-call width and
/// every call its line unbroken; else the chain as a block-indented
/// argument (`Ok(\n chain,\n)`). The head's limit is probed: rustfmt keeps a
/// 55-column head beside `Ok(` and moves a 61-column one.
pub fn push_ok_chain(output: &mut String, indent: usize, head: &str, calls: &[String]) {
    let pad = " ".repeat(indent);
    let chain = format!("{head}{}", calls.concat());
    let wide = calls.len() <= 1;
    if pad.len() + "Ok()".len() + chain.len() <= max_width() && (wide || chain.len() <= CHAIN_WIDTH)
    {
        output.push_str(&format!("{pad}Ok({chain})\n"));
        return;
    }
    let mut overflow = format!("{pad}Ok({head}\n");
    for (index, call) in calls.iter().enumerate() {
        let suffix = if index + 1 == calls.len() { ")" } else { "" };
        push_chain_call(&mut overflow, indent + 4, call, suffix);
    }
    let unbroken = overflow.lines().count() == calls.len() + 1;
    let fits = overflow.lines().all(|line| line.len() <= max_width());
    if head.len() <= FN_CALL_WIDTH && unbroken && fits {
        output.push_str(&overflow);
        return;
    }
    output.push_str(&format!("{pad}Ok(\n"));
    push_wide_chain(output, indent + 4, "", head, calls, ",");
    output.push_str(&format!("{pad})\n"));
}

/// `impl From<Sort<{key}>> for rok_data::Order<{row}>`, so a chain takes a
/// sort as it is: `.order_by(sort)`. `key` has an `order(direction)`.
pub fn push_sort_into_order(output: &mut String, key: &str, row: &str) {
    output.push_str("\n/// A sort as the chain's order: `.order_by(sort)`.\n");
    let head = format!("impl From<Sort<{key}>> for rok_data::Order<{row}> {{");
    if head.len() <= max_width() {
        output.push_str(&format!("{head}\n"));
    } else {
        output.push_str(&format!(
            "impl From<Sort<{key}>>\n    for rok_data::Order<{row}>\n{{\n"
        ));
    }
    let from = format!("    fn from(sort: Sort<{key}>) -> Self {{");
    if from.len() <= max_width() {
        output.push_str(&format!("{from}\n"));
    } else {
        output.push_str(&format!(
            "    fn from(\n        sort: Sort<{key}>,\n    ) -> Self {{\n"
        ));
    }
    output.push_str("        sort.key.order(sort.direction)\n    }\n}\n");
}

/// The arguments of `name(arg, ...)` as written, split on top-level
/// commas; `None` for calls without a parenthesized list (`.await`).
fn call_args(call: &str) -> Option<Vec<&str>> {
    let open = call.find('(')?;
    let inner = call.strip_suffix(')')?;
    let body = &inner[open + 1..];
    let mut args = Vec::new();
    if body.trim().is_empty() {
        return Some(args);
    }
    let mut rest = body;
    loop {
        let (mut depth, mut split_at) = (0isize, usize::MAX);
        for (index, byte) in rest.bytes().enumerate() {
            match byte {
                b'(' => depth += 1,
                b')' => depth -= 1,
                b',' if depth == 0 => {
                    split_at = index;
                    break;
                }
                _ => {}
            }
        }
        if split_at == usize::MAX {
            args.push(rest.trim());
            return Some(args);
        }
        args.push(rest[..split_at].trim());
        rest = &rest[split_at + 1..];
    }
}

/// One chain call at `pad` with `suffix` after it. A call with several
/// arguments frames them one per line when their written form exceeds the
/// function-call width; a one-argument call stays on its line while the
/// line fits [`MAX_WIDTH`] (rustfmt frames it only when the element itself
/// overflows the line).
fn push_chain_call(output: &mut String, pad: usize, call: &str, suffix: &str) {
    let pad = " ".repeat(pad);
    let args = match call_args(call) {
        Some(args) if !args.is_empty() => args,
        // `.await` and other calls without a parenthesized list stay on
        // their line.
        _ => {
            output.push_str(&format!("{pad}{call}{suffix}\n"));
            return;
        }
    };
    let framed = if args.len() > 1 {
        call_argument_width(call) > FN_CALL_WIDTH
    } else {
        pad.len() + call.len() + suffix.len() > max_width()
    };
    if !framed {
        output.push_str(&format!("{pad}{call}{suffix}\n"));
        return;
    }
    let open = call.find('(').expect("a chain call has arguments");
    output.push_str(&format!("{pad}{}(\n", &call[..open]));
    for arg in &args {
        push_chain_argument(output, &pad, arg);
    }
    output.push_str(&format!("{pad}){suffix}\n"));
}

/// The written width of a call's arguments: everything inside its
/// parentheses, verbatim.
fn call_argument_width(call: &str) -> usize {
    match (call.find('('), call.rfind(')')) {
        (Some(open), Some(close)) => close - open - 1,
        _ => 0,
    }
}

/// One framed argument at `pad`: its own line with a trailing comma, or,
/// when that line would overflow, its head followed by one `.link` per
/// line.
fn push_chain_argument(output: &mut String, pad: &str, arg: &str) {
    if pad.len() + 4 + arg.len() <= max_width() {
        output.push_str(&format!("{pad}    {arg},\n"));
        return;
    }
    // A chain argument breaks after its leader: the first `.name(` in it
    // opens the argument's own chain.
    let bytes = arg.as_bytes();
    let mut link = None;
    for at in 0..bytes.len() {
        if bytes[at] != b'.' {
            continue;
        }
        let mut end = at + 1;
        while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
            end += 1;
        }
        if end > at + 1 && bytes.get(end) == Some(&b'(') {
            link = Some(at);
            break;
        }
    }
    match link {
        Some(at) => {
            output.push_str(&format!("{pad}    {},\n", arg[..at].trim_end()));
            output.push_str(&format!("{pad}        {},\n", &arg[at..]));
        }
        None => output.push_str(&format!("{pad}    {arg},\n")),
    }
}

/// The derives `derive = ["serde"]` adds (engine ADR 0013), for [`push_derives`]:
/// none without `serde`.
pub fn serde_derives(serde: bool) -> &'static [&'static str] {
    if serde {
        &["serde::Serialize", "serde::Deserialize"]
    } else {
        &[]
    }
}

/// What a run's `derive` asks of the generated items: serde's derives
/// (engine ADR 0013), and `rok_data::Input` with its rules on create inputs and
/// update patches (engine ADR 0015), and with `[api] openapi`, utoipa's
/// `ToSchema` on the types an API feature exposes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeriveOptions {
    /// `derive = ["serde"]`.
    pub serde: bool,
    /// `derive = ["validate"]`.
    pub validate: bool,
    /// `api.output` with `[api] openapi` on: the typed ids and enums derive
    /// `utoipa::ToSchema`.
    pub openapi: bool,
}

impl DeriveOptions {
    /// The options `derive` lists, and `openapi`.
    pub fn of(derive: &[Derive], openapi: bool) -> Self {
        DeriveOptions {
            serde: derive.contains(&Derive::Serde),
            validate: derive.contains(&Derive::Validate),
            openapi,
        }
    }

    /// The derives of a typed id or an enum, which the API may expose:
    /// serde's, then `utoipa::ToSchema` with `openapi`.
    pub fn value_derives(self) -> Vec<&'static str> {
        let mut derives = serde_derives(self.serde).to_vec();
        derives.extend(super::openapi::to_schema(self.openapi));
        derives
    }

    /// The derives a create input or update patch adds: serde's, then
    /// `rok_data::Input` with `validate`.
    pub fn input_derives(self) -> Vec<&'static str> {
        let mut derives = serde_derives(self.serde).to_vec();
        if self.validate {
            derives.push("rok_data::Input");
        }
        derives
    }
}

/// `template` with the first `pattern` replaced by `with`. A template
/// written without `pattern` is a bug in the template or in its patch, so a
/// debug build panics instead of leaving the text silently unpatched; each
/// patched template has a unit test running its patches.
pub fn replace_once(template: &str, pattern: &str, with: &str) -> String {
    debug_assert!(
        template.contains(pattern),
        "the template no longer holds {pattern:?}"
    );
    template.replacen(pattern, with, 1)
}

/// Adds `extra` derives to the `#[derive(...)]` line `output` ends with, laid
/// out as rustfmt lays out a derive list: on one line when it fits,
/// otherwise one derive per line. No `extra` derive leaves `output` as is.
pub fn push_derives(output: &mut String, extra: &[&str]) {
    if extra.is_empty() {
        return;
    }
    let without_newline = output.strip_suffix('\n').unwrap_or(output);
    let start = without_newline.rfind('\n').map_or(0, |index| index + 1);
    let derives = output[start..]
        .trim_end()
        .strip_prefix("#[derive(")
        .and_then(|rest| rest.strip_suffix(")]"))
        .expect("derives follow a `#[derive(...)]` line")
        .to_owned();
    output.truncate(start);
    let derives = format!("{derives}, {}", extra.join(", "));
    // rustfmt keeps the list on one line only up to 96 columns, then on one
    // indented line up to 100, then one per line.
    let one_line = format!("#[derive({derives})]");
    let packed = format!("    {derives},");
    if one_line.len() <= max_width() - 4 {
        output.push_str(&one_line);
        output.push('\n');
    } else if packed.len() <= max_width() {
        output.push_str(&format!("#[derive(\n{packed}\n)]\n"));
    } else {
        output.push_str("#[derive(\n");
        for derive in derives.split(", ") {
            output.push_str(&format!("    {derive},\n"));
        }
        output.push_str(")]\n");
    }
}

/// Makes a serde-deriving search enum (a condition, `Direction`, or a sort
/// key) read and write its variants in snake_case, as JSON APIs expect
/// (`{"ge": 400}`, `"price_cents"`; engine ADR 0013). Generated enum types keep
/// their database labels instead.
pub fn push_serde_snake_case(output: &mut String) {
    output.push_str("#[serde(rename_all = \"snake_case\")]\n");
}

/// `use {prefix}::...;` for sorted `names`, wrapped like rustfmt wraps a use
/// list that does not fit on one line.
pub fn use_list(prefix: &str, names: &[String]) -> String {
    if let [name] = names {
        return format!("use {prefix}::{name};\n");
    }
    let one_line = format!("use {prefix}::{{{}}};", names.join(", "));
    // rustfmt keeps a use list on one line only up to 98 columns, and packs
    // the wrapped items greedily into lines of up to 99 columns.
    if one_line.len() < max_width() - 1 {
        return format!("{one_line}\n");
    }
    let mut output = format!("use {prefix}::{{\n");
    let mut line = String::new();
    for name in names {
        if !line.is_empty() && line.len() + 1 + name.len() + 1 > max_width() - 1 {
            output.push_str(&line);
            output.push('\n');
            line.clear();
        }
        if line.is_empty() {
            line.push_str(&format!("    {name},"));
        } else {
            line.push_str(&format!(" {name},"));
        }
    }
    output.push_str(&line);
    output.push_str("\n};\n");
    output
}

/// `use` lines for full paths (one item each), in rustfmt's order: `super`
/// segments before named modules.
pub fn use_paths(paths: &[String]) -> String {
    let mut sorted: Vec<&String> = paths.iter().collect();
    sorted.sort_by_key(|path| {
        path.split("::")
            .map(|segment| (segment != "super", segment.to_owned()))
            .collect::<Vec<_>>()
    });
    sorted.dedup();
    sorted.iter().map(|path| format!("use {path};\n")).collect()
}

/// `use {db_path}::types;` when any of the Rust types a file mentions is a
/// generated enum (`types::X`), else empty. `db_path` is the path from the
/// file's module to `db`, e.g. `super::super` from a resource file.
pub fn types_import<'a>(db_path: &str, rust_types: impl IntoIterator<Item = &'a str>) -> String {
    if rust_types.into_iter().any(mentions_generated_type) {
        format!("use {db_path}::types;\n")
    } else {
        String::new()
    }
}

/// A SQL name in backticks for a one-line doc comment: line breaks a
/// quoted identifier may hold become spaces.
pub fn doc_name(sql_name: &str) -> String {
    format!("`{}`", sql_name.replace(['\r', '\n'], " "))
}

/// Appends `/// line` doc comments at `indent`.
pub fn push_doc(output: &mut String, indent: usize, lines: &[&str]) {
    let pad = " ".repeat(indent);
    for line in lines {
        if line.is_empty() {
            output.push_str(&format!("{pad}///\n"));
        } else {
            output.push_str(&format!("{pad}/// {line}\n"));
        }
    }
}

/// Appends a generated query function's head: the signature (one line when
/// it fits, otherwise one parameter per line), the shared executor bound, and
/// the opening brace.
pub fn push_fn_head(output: &mut String, name: &str, params: &[String], returns: &str) {
    let mut all_params = vec!["executor: E".to_owned()];
    all_params.extend(params.iter().cloned());

    let open = format!("pub async fn {name}<'e, E>(");
    let close = format!(") -> {returns}");
    let one_line = format!("{open}{}{close}", all_params.join(", "));
    if one_line.len() <= max_width() {
        output.push_str(&one_line);
        output.push('\n');
    } else {
        output.push_str(&open);
        output.push('\n');
        for param in &all_params {
            output.push_str(&format!("    {param},\n"));
        }
        output.push_str(&close);
        output.push('\n');
    }
    output.push_str("where\n    E: rok_data::Executor<'e>,\n{\n");
}

/// Renders `a && b && c` at `indent`, wrapped the way rustfmt wraps it.
/// `lead` and `trail` surround the chain (`"if "` and `" {"` for a
/// condition); a wrapped condition puts its opening brace on its own line.
pub fn push_and_chain(
    output: &mut String,
    indent: usize,
    lead: &str,
    terms: &[String],
    trail: &str,
) {
    let pad = " ".repeat(indent);
    let one_line = format!("{pad}{lead}{}{trail}", terms.join(" && "));
    if one_line.len() <= max_width() {
        output.push_str(&one_line);
        output.push('\n');
        return;
    }
    output.push_str(&format!("{pad}{lead}{}\n", terms[0]));
    for term in &terms[1..] {
        output.push_str(&format!("{pad}    && {term}\n"));
    }
    let trail = trail.trim_start();
    if !trail.is_empty() {
        output.push_str(&format!("{pad}{trail}\n"));
    }
}

/// Appends `{binding} = {call}({argument});` at `indent`, laid out the way
/// rustfmt lays it out: on one line when it fits, otherwise with the call
/// moved below the `=`, otherwise with the argument on its own line.
pub fn push_let_call(
    output: &mut String,
    indent: usize,
    binding: &str,
    call: &str,
    argument: &str,
) {
    let pad = " ".repeat(indent);
    let one_line = format!("{pad}{binding} = {call}({argument});");
    let below = format!("{pad}    {call}({argument});");
    if one_line.len() <= max_width() {
        output.push_str(&one_line);
        output.push('\n');
    } else if below.len() <= max_width() {
        output.push_str(&format!("{pad}{binding} =\n{below}\n"));
    } else {
        output.push_str(&format!(
            "{pad}{binding} = {call}(\n{pad}    {argument},\n{pad});\n"
        ));
    }
}

/// A statement whose SQL is fully known at generation time.
pub struct StaticQuery<'a> {
    /// The SQLx entry point, e.g. `sqlx::query_as::<_, Product>`.
    pub call: &'a str,
    pub sql: Vec<String>,
    /// Bind expressions in placeholder order: the n-th entry binds `$n`.
    pub binds: Vec<String>,
    /// The execution method call, e.g. `fetch_optional(executor)`.
    pub execute: &'a str,
}

impl StaticQuery<'_> {
    /// Appends the query as a method chain at `indent`, preceded by `lead`
    /// (such as `let result = ` or `return `) and ending with `.await` plus
    /// `trail` (such as `?;`).
    pub fn push(&self, output: &mut String, indent: usize, lead: &str, trail: &str) {
        let pad = " ".repeat(indent);
        output.push_str(&format!("{pad}{lead}{}(\n", self.call));
        output.push_str(&sql::raw_block(&self.sql, indent + 4));
        output.push_str(",\n");
        output.push_str(&format!("{pad})\n"));
        for bind in &self.binds {
            output.push_str(&format!("{pad}.bind({bind})\n"));
        }
        output.push_str(&format!("{pad}.{}\n", self.execute));
        output.push_str(&format!("{pad}.await{trail}\n"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// rustfmt merges adjacent derives, so serde joins the existing list,
    /// wrapped one per line once it is too wide.
    #[test]
    fn serde_derives_join_the_derive_list_as_rustfmt_lays_it_out() {
        let mut short = "/// Doc.\n#[derive(Debug, Clone)]\n".to_owned();
        push_derives(&mut short, serde_derives(true));
        assert_eq!(
            short,
            "/// Doc.\n#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\n"
        );

        let mut long =
            "#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, sqlx::Type)]\n"
                .to_owned();
        push_derives(&mut long, serde_derives(true));
        assert_eq!(
            long,
            "#[derive(\n    Debug,\n    Clone,\n    Copy,\n    PartialEq,\n    Eq,\n    PartialOrd,\n    Ord,\n    Hash,\n    sqlx::Type,\n    serde::Serialize,\n    serde::Deserialize,\n)]\n"
        );

        let mut packed =
            "#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]\n".to_owned();
        push_derives(&mut packed, serde_derives(true));
        assert_eq!(
            packed,
            "#[derive(\n    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,\n)]\n"
        );

        let mut off = "#[derive(Debug)]\n".to_owned();
        push_derives(&mut off, serde_derives(false));
        assert_eq!(off, "#[derive(Debug)]\n");
    }

    /// The layouts rustfmt was probed for: one line, the head beside `Ok(`,
    /// and a block-indented argument once the head passes the call width.
    #[test]
    fn ok_chains_lay_out_as_rustfmt_does() {
        let calls = |calls: &[&str]| {
            calls
                .iter()
                .map(|call| (*call).to_owned())
                .collect::<Vec<_>>()
        };
        let mut short = String::new();
        push_ok_chain(
            &mut short,
            8,
            "Note::search(filter)?",
            &calls(&[".order_by(Note::ID)"]),
        );
        assert_eq!(
            short,
            "        Ok(Note::search(filter)?.order_by(Note::ID))\n"
        );

        let links = calls(&[
            ".order_by(sort.key.order(sort.direction))",
            ".order_by(Q::ID)",
        ]);
        let mut beside = String::new();
        push_ok_chain(
            &mut beside,
            4,
            "QuarterlyFinancialStatement::search(tenant_id, filter)?",
            &links,
        );
        assert_eq!(
            beside,
            "    Ok(QuarterlyFinancialStatement::search(tenant_id, filter)?\n        .order_by(sort.key.order(sort.direction))\n        .order_by(Q::ID))\n"
        );

        let mut block = String::new();
        push_ok_chain(
            &mut block,
            4,
            "QuarterlyFinancialStatementDocuments::search(tenant, filter)?",
            &links,
        );
        assert_eq!(
            block,
            "    Ok(\n        QuarterlyFinancialStatementDocuments::search(tenant, filter)?\n            .order_by(sort.key.order(sort.direction))\n            .order_by(Q::ID),\n    )\n"
        );
    }

    #[test]
    fn fn_head_stays_on_one_line_when_it_fits() {
        let mut output = String::new();
        push_fn_head(&mut output, "list", &[], "Result<(), sqlx::Error>");
        assert_eq!(
            output,
            "pub async fn list<'e, E>(executor: E) -> Result<(), sqlx::Error>\nwhere\n    E: rok_data::Executor<'e>,\n{\n"
        );
    }

    #[test]
    fn fn_head_goes_vertical_when_too_wide() {
        let mut output = String::new();
        let params = vec![
            "order_id: uuid::Uuid".to_owned(),
            "product_id: uuid::Uuid".to_owned(),
        ];
        push_fn_head(
            &mut output,
            "get_by_id",
            &params,
            "Result<Option<LineItem>, sqlx::Error>",
        );
        assert!(output.starts_with(
            "pub async fn get_by_id<'e, E>(\n    executor: E,\n    order_id: uuid::Uuid,\n    product_id: uuid::Uuid,\n) -> Result<Option<LineItem>, sqlx::Error>\nwhere\n"
        ));
    }
}
