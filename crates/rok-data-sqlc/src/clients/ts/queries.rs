//! `src/queries.ts`: the TanStack Query factories. Every `GET` gets a
//! `queryOptions` factory taking its call's arguments — its key is
//! `[group, name, ...arguments]`, so `keys.<group>` prefixes every key of
//! a group — and every other operation a `mutationOptions` factory taking
//! `RequestOptions`, whose variables are its call's one argument, or an
//! object of them by name when it takes several.

use super::calls::{Argument, arguments, groups, import_types, used_types};
use super::{Named, TypeNames};

/// The text of `src/queries.ts` (after the header).
pub(super) fn file(operations: &[Named<'_>], names: &TypeNames) -> String {
    let all: Vec<&Named<'_>> = operations.iter().collect();
    let reads: Vec<&Named<'_>> = all
        .iter()
        .copied()
        .filter(|named| named.is_query())
        .collect();
    let writes: Vec<&Named<'_>> = all
        .iter()
        .copied()
        .filter(|named| !named.is_query())
        .collect();

    let mut imports = Vec::new();
    if !writes.is_empty() {
        imports.push("mutationOptions");
    }
    if !reads.is_empty() {
        imports.push("queryOptions");
    }
    let mut out = String::from("\n");
    if !imports.is_empty() {
        out.push_str(&format!(
            "import {{ {} }} from \"@tanstack/react-query\"\n",
            imports.join(", ")
        ));
    }
    out.push_str("import { api } from \"./calls\"\n");
    if !writes.is_empty() {
        out.push_str("import type { RequestOptions } from \"./runtime\"\n");
    }
    let mut used = used_types(&reads, names, false);
    used.extend(used_types(&writes, names, false));
    out.push_str(&import_types(&used));

    out.push_str("\n/** The prefix of every query key of a tag: invalidate by it. */\nexport const keys = {\n");
    for (group, _) in groups(&all) {
        out.push_str(&format!("  {group}: [{group:?}] as const,\n"));
    }
    out.push_str("}\n");

    out.push_str(
        "\n/** `queryOptions` for every `GET`, by tag: `useQuery(queries.users.list())`. */\nexport const queries = {\n",
    );
    for (group, members) in groups(&reads) {
        out.push_str(&format!("  {group}: {{\n"));
        for named in members {
            out.push_str(&query(named, names));
        }
        out.push_str("  },\n");
    }
    out.push_str("}\n");

    out.push_str(
        "\n/**\n * `mutationOptions` for every other operation, by tag:\n * `useMutation(mutations.users.update())`.\n */\nexport const mutations = {\n",
    );
    for (group, members) in groups(&writes) {
        out.push_str(&format!("  {group}: {{\n"));
        for named in members {
            out.push_str(&mutation(named, names));
        }
        out.push_str("  },\n");
    }
    out.push_str("}\n");
    out
}

/// One `queryOptions` factory.
fn query(named: &Named<'_>, names: &TypeNames) -> String {
    let arguments = arguments(named, names);
    let declarations: Vec<String> = arguments.iter().map(Argument::declaration).collect();
    let mut key = vec![format!("{:?}", named.group), format!("{:?}", named.name)];
    let mut call = Vec::new();
    for argument in &arguments {
        key.push(if argument.optional {
            format!("{} ?? {{}}", argument.name)
        } else {
            argument.name.clone()
        });
        call.push(argument.name.clone());
    }
    call.push("{ signal }".to_owned());
    format!(
        "    {name}: ({declarations}) =>\n      queryOptions({{\n        queryKey: [{key}] as const,\n        queryFn: ({{ signal }}) => api.{group}.{name}({call}),\n      }}),\n",
        name = named.name,
        group = named.group,
        declarations = declarations.join(", "),
        key = key.join(", "),
        call = call.join(", "),
    )
}

/// One `mutationOptions` factory.
fn mutation(named: &Named<'_>, names: &TypeNames) -> String {
    let arguments = arguments(named, names);
    let (variables, call): (String, Vec<String>) = match arguments.as_slice() {
        [] => (String::new(), Vec::new()),
        [only] => (only.declaration(), vec![only.name.clone()]),
        several => (
            format!(
                "variables: {{ {} }}",
                several
                    .iter()
                    .map(Argument::declaration)
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
            several
                .iter()
                .map(|argument| format!("variables.{}", argument.name))
                .collect(),
        ),
    };
    let mut call = call;
    call.push("options".to_owned());
    format!(
        "    {name}: (options?: RequestOptions) =>\n      mutationOptions({{\n        mutationKey: [{group:?}, {name:?}] as const,\n        mutationFn: ({variables}) =>\n          api.{group}.{name}({call}),\n      }}),\n",
        name = named.name,
        group = named.group,
        variables = variables,
        call = call.join(", "),
    )
}
