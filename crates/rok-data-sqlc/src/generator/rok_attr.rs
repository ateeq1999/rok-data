//! The `#[rok(...)]` field attributes of `#[derive(rok_data::Input)]`
//! (workspace ADR 0021), laid out as rustfmt lays them out.

use super::MAX_WIDTH;

/// rustfmt's default `attr_fn_like_width`: the arguments of an attribute's
/// list longer than this go one per line.
const ATTR_FN_LIKE_WIDTH: usize = 70;

/// How wide a field's attribute rustfmt keeps on one line: three short of
/// the maximum width (found by running rustfmt; a 98-column `#[rok(…)]`
/// breaks).
const ATTR_MAX_WIDTH: usize = MAX_WIDTH - 3;

/// One item of an attribute: a word (`trim`, `min = 1`) or a list
/// (`validate(…)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// `trim`, `min = 1`, `regex = "^a$"`.
    Word(String),
    /// `validate(…)`: a name and its items.
    List(String, Vec<Item>),
}

impl Item {
    /// A word.
    pub fn word(text: impl Into<String>) -> Self {
        Item::Word(text.into())
    }

    /// A list.
    pub fn list(name: impl Into<String>, items: Vec<Item>) -> Self {
        Item::List(name.into(), items)
    }

    /// The item on one line.
    pub(crate) fn one_line(&self) -> String {
        match self {
            Item::Word(text) => text.clone(),
            Item::List(name, items) => {
                let items: Vec<String> = items.iter().map(Item::one_line).collect();
                format!("{name}({})", items.join(", "))
            }
        }
    }

    /// Whether rustfmt formats the item: it formats an attribute only when
    /// every value is a literal, and leaves one with a path or an array
    /// value (`function = path`, `values = [...]`) as written.
    fn formattable(&self) -> bool {
        match self {
            Item::Word(text) => text.split_once(" = ").is_none_or(|(_, value)| {
                value.starts_with('"')
                    || value.starts_with('-')
                    || value.starts_with(|c: char| c.is_ascii_digit())
            }),
            Item::List(_, items) => items.iter().all(Item::formattable),
        }
    }

    /// Whether rustfmt keeps the item's arguments on one line: a list's
    /// arguments break past `attr_fn_like_width` (70), except a list of one
    /// list, which leaves it to the inner one.
    fn short(&self) -> bool {
        match self {
            Item::Word(_) => true,
            Item::List(_, items) if items.len() == 1 && matches!(items[0], Item::List(..)) => {
                items[0].short()
            }
            Item::List(_, items) => {
                let args: Vec<String> = items.iter().map(Item::one_line).collect();
                args.join(", ").len() <= ATTR_FN_LIKE_WIDTH && items.iter().all(Item::short)
            }
        }
    }

    /// The item starting at `column`, its lines after the first indented by
    /// `indent`, with `trailing` characters still to follow on its last
    /// line: on one line when it fits within `width`; a list of one list
    /// overflows into it; otherwise one item per line.
    fn layout(&self, indent: usize, column: usize, trailing: usize, width: usize) -> String {
        let one = self.one_line();
        if column + one.len() + trailing <= width && self.short() {
            return one;
        }
        match self {
            Item::Word(_) => one,
            Item::List(name, items) if items.len() == 1 && matches!(items[0], Item::List(..)) => {
                let inner = items[0].layout(indent, column + name.len() + 1, trailing + 1, width);
                format!("{name}({inner})")
            }
            Item::List(name, items) => {
                let pad = indent + 4;
                let mut text = format!("{name}(\n");
                for (index, item) in items.iter().enumerate() {
                    let last = index + 1 == items.len();
                    text.push_str(&" ".repeat(pad));
                    text.push_str(&item.layout(pad, pad, usize::from(!last), MAX_WIDTH));
                    text.push_str(if last { "\n" } else { ",\n" });
                }
                text.push_str(&" ".repeat(indent));
                text.push(')');
                text
            }
        }
    }
}

/// Appends a field's `#[rok(items…)]`, indented as a field's attribute;
/// nothing without items.
pub fn push_rok(output: &mut String, items: Vec<Item>) {
    if items.is_empty() {
        return;
    }
    let attr = Item::list("rok", items);
    let line = format!("    #[{}]", attr.one_line());
    if (line.len() <= ATTR_MAX_WIDTH && attr.short()) || !attr.formattable() {
        output.push_str(&line);
    } else {
        output.push_str("    #[");
        output.push_str(&attr.layout(4, 6, 1, ATTR_MAX_WIDTH));
        output.push(']');
    }
    output.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(items: Vec<Item>) -> String {
        let mut output = String::new();
        push_rok(&mut output, items);
        output
    }

    fn length(message: &str) -> Item {
        Item::list(
            "length",
            vec![
                Item::word("min = 1"),
                Item::word("max = 80"),
                Item::word(format!("message = \"{message}\"")),
            ],
        )
    }

    #[test]
    fn an_attribute_that_fits_stays_on_one_line() {
        assert_eq!(
            rendered(vec![Item::list("validate", vec![length("short")])]),
            "    #[rok(validate(length(min = 1, max = 80, message = \"short\")))]\n"
        );
    }

    #[test]
    fn a_long_attribute_breaks_as_rustfmt_breaks_it() {
        let message = "name must be between 1 and 80 characters";
        assert_eq!(
            rendered(vec![Item::list("validate", vec![length(message)])]),
            format!(
                "    #[rok(validate(length(\n        min = 1,\n        max = 80,\n        message = \"{message}\"\n    )))]\n"
            )
        );
        assert_eq!(
            rendered(vec![
                Item::list(
                    "transform",
                    vec![Item::word("trim"), Item::word("lowercase")]
                ),
                Item::list("validate", vec![length(message)]),
            ]),
            format!(
                "    #[rok(\n        transform(trim, lowercase),\n        validate(length(\n            min = 1,\n            max = 80,\n            message = \"{message}\"\n        ))\n    )]\n"
            )
        );
    }

    #[test]
    fn a_field_attribute_breaks_three_short_of_the_width() {
        // 97 columns: one line; 98: broken (found by running rustfmt).
        let range = |message: &str| {
            Item::list(
                "range",
                vec![
                    Item::word("min = 1900"),
                    Item::word("max = 2100"),
                    Item::word(format!("message = \"{message}\"")),
                ],
            )
        };
        let kept = rendered(vec![Item::list("validate", vec![range(&"y".repeat(32))])]);
        assert_eq!(kept.len(), 98, "{kept}");
        assert_eq!(kept.lines().count(), 1, "{kept}");
        let broken = rendered(vec![Item::list("validate", vec![range(&"y".repeat(33))])]);
        assert_eq!(broken.lines().count(), 5, "{broken}");
    }

    #[test]
    fn a_path_value_keeps_the_attribute_as_written() {
        let custom = Item::list(
            "custom",
            vec![
                Item::word("function = immutable"),
                Item::word("code = \"immutable\""),
                Item::word("message = \"`email` can't be changed, not ever, by anyone\""),
            ],
        );
        let output = rendered(vec![Item::list("validate", vec![custom])]);
        assert_eq!(output.lines().count(), 1, "{output}");
    }
}
