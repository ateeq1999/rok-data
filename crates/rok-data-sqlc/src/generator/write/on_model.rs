//! The write verbs on the model (engine ADR 0018): the rendered functions
//! become associated functions of the row type — `Book::create(&db, input)`
//! — and the module keeps a deprecated wrapper per verb that forwards to it,
//! so 0.2 call sites (`books::write::create`) still compile, with a warning
//! naming the model verb.

/// The verbs `write.rs` puts on a model: a scope of that name would collide
/// with the associated function.
pub const MODEL_VERBS: &[&str] = &[
    "attach",
    "create",
    "delete_permanently",
    "destroy",
    "detach",
    "patch",
    "restore",
    "sync",
];

/// One rendered function: its doc and attribute lines, its signature lines
/// up to (not including) the body's opening brace, and whether it is async.
struct Function<'a> {
    signature: Vec<&'a str>,
    name: String,
    params: Vec<String>,
    is_async: bool,
}

/// Wraps `items` — functions and comments rendered at column 0 — in
/// `impl {model}`, then appends a deprecated module wrapper per function.
pub fn on_the_model(model: &str, items: &str) -> String {
    if items.trim().is_empty() {
        return items.to_owned();
    }
    let mut output = format!("\nimpl {model} {{");
    for line in items.trim_start_matches('\n').lines() {
        output.push('\n');
        if !line.is_empty() {
            output.push_str("    ");
            output.push_str(line);
        }
    }
    output.push_str("\n}\n");
    for function in functions(items) {
        let verb = format!("{model}::{}", function.name);
        output.push_str(&format!(
            "\n/// Calls [`{verb}`], which this module function forwards to.\n\
             #[deprecated(note = \"call `{verb}` instead\")]\n"
        ));
        for line in at_column_zero(&function.signature) {
            output.push_str(&line);
            output.push('\n');
        }
        let await_ = if function.is_async { ".await" } else { "" };
        // A one-line signature already ends with the body's opening brace.
        if !function
            .signature
            .last()
            .is_some_and(|line| line.ends_with('{'))
        {
            output.push_str("{\n");
        }
        output.push_str(&forwarding_call(&verb, &function.params, await_));
        output.push_str("}\n");
    }
    output
}

/// The wrapper's body, `{verb}({params}){await_}` at 4 columns, laid out
/// as rustfmt lays out a call: on one line while the arguments fit
/// `fn_call_width` and the line fits; else one argument per line.
fn forwarding_call(verb: &str, params: &[String], await_: &str) -> String {
    let joined = params.join(", ");
    let one_line = format!("    {verb}({joined}){await_}");
    if joined.len() <= super::super::emit::FN_CALL_WIDTH
        && one_line.len() <= super::super::emit::MAX_WIDTH
    {
        return format!("{one_line}\n");
    }
    let mut output = format!("    {verb}(\n");
    for param in params {
        output.push_str(&format!("        {param},\n"));
    }
    // rustfmt puts a chained `.await` on its own line after a broken call.
    if await_.is_empty() {
        output.push_str("    )\n");
    } else {
        output.push_str(&format!("    )\n    {await_}\n"));
    }
    output
}

/// A signature laid out for the `impl` (4 columns in), laid out again for
/// column 0: a parameter list broken one per line goes back on one line
/// when that fits [`MAX_WIDTH`](crate::generator::emit::MAX_WIDTH), as
/// rustfmt would put it.
fn at_column_zero(signature: &[&str]) -> Vec<String> {
    let close = signature.iter().position(|line| line.starts_with(')'));
    if let (true, Some(close)) = (signature[0].ends_with('('), close) {
        let params: Vec<&str> = signature[1..close]
            .iter()
            .map(|line| line.trim().trim_end_matches(','))
            .collect();
        let one_line = format!("{}{}{}", signature[0], params.join(", "), signature[close]);
        if one_line.len() <= super::super::emit::MAX_WIDTH {
            let mut lines = vec![one_line];
            lines.extend(signature[close + 1..].iter().map(|line| (*line).to_owned()));
            return lines;
        }
    }
    signature.iter().map(|line| (*line).to_owned()).collect()
}

/// The functions of `items`, in order. A function starts at a column-0
/// `pub fn` / `pub async fn` line and its body ends at a column-0 `}`.
fn functions(items: &str) -> Vec<Function<'_>> {
    let mut found = Vec::new();
    let mut lines = items.lines();
    while let Some(line) = lines.next() {
        let Some(rest) = line
            .strip_prefix("pub async fn ")
            .map(|rest| (rest, true))
            .or_else(|| line.strip_prefix("pub fn ").map(|rest| (rest, false)))
        else {
            continue;
        };
        let (rest, is_async) = rest;
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        let mut signature = vec![line];
        if !line.ends_with('{') {
            for next in lines.by_ref() {
                if next == "{" {
                    break;
                }
                signature.push(next);
                if next.ends_with('{') {
                    break;
                }
            }
        }
        for next in lines.by_ref() {
            if next == "}" {
                break;
            }
        }
        let params = params(&signature);
        found.push(Function {
            signature,
            name,
            params,
            is_async,
        });
    }
    found
}

/// The parameter names of a rendered signature, read with `syn`.
fn params(signature: &[&str]) -> Vec<String> {
    let text = signature.join("\n");
    let text = text.trim_end().trim_end_matches('{');
    let text = text.strip_prefix("pub ").unwrap_or(text);
    let Ok(parsed) = syn::parse_str::<syn::Signature>(text) else {
        unreachable!("the engine renders valid signatures: {text}");
    };
    parsed
        .inputs
        .iter()
        .filter_map(|input| match input {
            syn::FnArg::Typed(typed) => match &*typed.pat {
                syn::Pat::Ident(ident) => Some(ident.ident.to_string()),
                _ => None,
            },
            syn::FnArg::Receiver(_) => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ITEMS: &str = "
/// Inserts one row and returns it as stored.
pub async fn create<'e, E>(executor: E, input: NewBook) -> Result<Book, rok_data::Error>
where
    E: rok_data::Executor<'e>,
{
    Book::insert_builder().exec(executor).await
}

// No updatable columns detected; patch is not generated for this table.
";

    #[test]
    fn the_verbs_move_onto_the_model_and_the_module_forwards() {
        let rendered = on_the_model("Book", ITEMS);
        assert!(rendered.starts_with(
            "\nimpl Book {\n    /// Inserts one row and returns it as stored.\n    pub async fn create<'e, E>("
        ), "{rendered}");
        assert!(rendered.contains(
            "\n    // No updatable columns detected; patch is not generated for this table.\n}\n"
        ), "{rendered}");
        assert!(rendered.ends_with(
            "\n/// Calls [`Book::create`], which this module function forwards to.\n\
             #[deprecated(note = \"call `Book::create` instead\")]\n\
             pub async fn create<'e, E>(executor: E, input: NewBook) -> Result<Book, rok_data::Error>\n\
             where\n    E: rok_data::Executor<'e>,\n{\n    Book::create(executor, input).await\n}\n"
        ), "{rendered}");
    }

    #[test]
    fn a_wrapper_rejoins_a_signature_that_fits_at_column_zero() {
        // 97 columns on one line: over the impl's budget (96), so the
        // renderer broke it; whole again at column 0.
        let one_line = "pub async fn patch<'e, E>(executor: E, input: UpdateWarehouseRows) -> Result<Wh, rok_data::Error>";
        assert_eq!(one_line.len(), 97);
        let items = "\npub async fn patch<'e, E>(\n    executor: E,\n    input: UpdateWarehouseRows,\n) -> Result<Wh, rok_data::Error>\nwhere\n    E: rok_data::Executor<'e>,\n{\n    todo()\n}\n";
        let rendered = on_the_model("Wh", items);
        assert!(
            rendered.ends_with(&format!(
                "{one_line}\nwhere\n    E: rok_data::Executor<'e>,\n{{\n    Wh::patch(executor, input).await\n}}\n"
            )),
            "{rendered}"
        );
    }

    #[test]
    fn a_one_line_signature_keeps_its_brace() {
        let items = "\n/// Pairs them.\npub async fn attach(executor: E, a: i64) -> Result<bool> {\n    todo()\n}\n";
        let rendered = on_the_model("Pair", items);
        assert!(rendered.ends_with(
            "pub async fn attach(executor: E, a: i64) -> Result<bool> {\n    Pair::attach(executor, a).await\n}\n"
        ), "{rendered}");
    }
}
