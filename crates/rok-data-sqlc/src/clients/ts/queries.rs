//! `src/queries.ts`: the TanStack Query factories. Every `GET` gets a
//! `queryOptions` factory taking its call's arguments — its key is
//! `[group, name, ...arguments]`, so `keys.<group>` prefixes every key of
//! a group — and every other operation a `mutationOptions` factory taking
//! `RequestOptions`, whose variables are its call's one argument, or an
//! object of them by name when it takes several. An operation's header
//! parameters are a `GET` factory's last argument (`headers`), and the
//! fields of a mutation's variables.

use super::super::openapi::{Document, Paging};
use super::calls::{Argument, arguments, groups, import_types, options_required, used_types};
use super::{Named, TypeNames, header_field};

/// The text of `src/queries.ts` (after the header).
pub(super) fn file(document: &Document, operations: &[Named<'_>], names: &TypeNames) -> String {
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
    let paged: Vec<(&Named<'_>, Paging)> = reads
        .iter()
        .filter(|named| named.op.header_params.is_empty())
        .filter_map(|named| document.paging(named.op).map(|paging| (*named, paging)))
        .collect();

    let mut imports = Vec::new();
    if !paged.is_empty() {
        imports.push("infiniteQueryOptions");
    }
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
    out.push_str(match (paged.is_empty(), writes.is_empty()) {
        (false, false) => "import { nextPage, type RequestOptions } from \"./runtime\"\n",
        (false, true) => "import { nextPage } from \"./runtime\"\n",
        (true, false) => "import type { RequestOptions } from \"./runtime\"\n",
        (true, true) => "",
    });
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

    out.push_str(
        "\n/**\n * `infiniteQueryOptions` for every list that pages, by tag:\n * `useInfiniteQuery(pagers.users.list())`.\n */\nexport const pagers = {\n",
    );
    let lists: Vec<&Named<'_>> = paged.iter().map(|(named, _)| *named).collect();
    for (group, members) in groups(&lists) {
        out.push_str(&format!("  {group}: {{\n"));
        for named in members {
            if let Some((_, paging)) = paged.iter().find(|(list, _)| std::ptr::eq(*list, named)) {
                out.push_str(&pager(named, *paging, names));
            }
        }
        out.push_str("  },\n");
    }
    out.push_str("}\n");
    out
}

/// One `infiniteQueryOptions` factory: the list from the query's page (or
/// cursor) on, each next page the one `nextPage` reads off the last.
fn pager(named: &Named<'_>, paging: Paging, names: &TypeNames) -> String {
    let arguments = arguments(named, names);
    let declarations: Vec<String> = arguments.iter().map(Argument::declaration).collect();
    let (field, start) = match paging {
        Paging::Numbered => ("page", "query?.page ?? 1"),
        Paging::Cursor => ("cursor", "query?.cursor"),
    };
    let mut key = vec![
        format!("{:?}", named.group),
        format!("{:?}", named.name),
        "\"pages\"".to_owned(),
    ];
    let mut call = Vec::new();
    for argument in &arguments {
        if argument.name == "query" {
            key.push("query ?? {}".to_owned());
            call.push(format!("{{ ...query, {field}: pageParam }}"));
        } else {
            key.push(argument.name.clone());
            call.push(argument.name.clone());
        }
    }
    call.push("{ signal }".to_owned());
    format!(
        "    {name}: ({declarations}) =>\n      infiniteQueryOptions({{\n        \
         queryKey: [{key}] as const,\n        \
         queryFn: ({{ pageParam, signal }}) =>\n          api.{group}.{name}({call}),\n        \
         initialPageParam: {start},\n        \
         getNextPageParam: (last) => nextPage(last),\n      }}),\n",
        name = named.name,
        group = named.group,
        declarations = declarations.join(", "),
        key = key.join(", "),
        call = call.join(", "),
    )
}

/// One `queryOptions` factory.
fn query(named: &Named<'_>, names: &TypeNames) -> String {
    let mut arguments = arguments(named, names);
    let headers = !named.op.header_params.is_empty();
    if headers {
        arguments.push(Argument {
            name: "headers".to_owned(),
            ty: named.headers_type(),
            optional: !options_required(named),
        });
    }
    let declarations: Vec<String> = arguments.iter().map(Argument::declaration).collect();
    let mut key = vec![format!("{:?}", named.group), format!("{:?}", named.name)];
    let mut call = Vec::new();
    for argument in &arguments {
        if headers && argument.name == "headers" {
            key.push(if argument.optional {
                "headers ?? {}".to_owned()
            } else {
                "headers".to_owned()
            });
            continue;
        }
        key.push(if argument.optional {
            format!("{} ?? {{}}", argument.name)
        } else {
            argument.name.clone()
        });
        call.push(argument.name.clone());
    }
    call.push(if headers {
        "{ ...headers, signal }".to_owned()
    } else {
        "{ signal }".to_owned()
    });
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
    if !named.op.header_params.is_empty() {
        return mutation_with_headers(named, &arguments);
    }
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

/// One `mutationOptions` factory of an operation taking header
/// parameters: its variables are its arguments by name and its headers'
/// fields, which go to the call's options.
fn mutation_with_headers(named: &Named<'_>, arguments: &[Argument]) -> String {
    let headers = named.headers_type();
    let variables = if arguments.is_empty() {
        headers
    } else {
        format!(
            "{{ {} }} & {headers}",
            arguments
                .iter()
                .map(Argument::declaration)
                .collect::<Vec<_>>()
                .join("; ")
        )
    };
    let mut call: Vec<String> = arguments
        .iter()
        .map(|argument| format!("variables.{}", argument.name))
        .collect();
    call.push("{".to_owned());
    let fields: String = named
        .op
        .header_params
        .iter()
        .map(|parameter| {
            let field = header_field(&parameter.name);
            format!("            {field}: variables.{field},\n")
        })
        .collect();
    format!(
        "    {name}: (options?: RequestOptions) =>\n      mutationOptions({{\n        \
         mutationKey: [{group:?}, {name:?}] as const,\n        mutationFn: (\n          \
         variables: {variables}\n        ) =>\n          api.{group}.{name}({call}\n            \
         ...options,\n{fields}          }}),\n      }}),\n",
        name = named.name,
        group = named.group,
        call = call.join(", "),
    )
}
