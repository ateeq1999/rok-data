//! A feature's gRPC service under `framework = "grpc"` (engine ADR 0028):
//! `grpc.rs`, in place of the routes and handlers, implements the tonic
//! service `proto/<resource>.proto` declares on the feature's own service.
//! Each RPC reads its request message into the feature's DTOs, calls one
//! method of the service trait the HTTP handlers call, and answers with
//! messages; `shared/grpc.rs` turns the values and the errors.
//!
//! The root's `generated.rs` includes what `build.rs` compiles from
//! `proto/`, and mounts every feature's service in one `Routes`.

use heck::{ToSnakeCase, ToUpperCamelCase};

use super::dto::{create_columns, update_columns};
use super::feature::{Endpoint, Feature, Key};
use super::layout::{Uses, push_call, push_signature};
use crate::ApiAccess;
use crate::generator::{HEADER, MAX_WIDTH, push_doc};
use crate::schema::Column;

/// rustfmt's default `chain_width`.
const CHAIN_WIDTH: usize = 60;

/// The RPCs of the proto's service, in its order: the RPC, the endpoint
/// it serves, and the request and response messages.
fn rpcs(feature: &Feature<'_>) -> Vec<(&'static str, Endpoint, String, String)> {
    let row = message(feature.row());
    let plural = feature.module().to_upper_camel_case();
    [
        (
            "get",
            Endpoint::GetOne,
            format!("Get{row}Request"),
            row.clone(),
        ),
        (
            "list",
            Endpoint::List,
            format!("List{plural}Request"),
            format!("List{plural}Response"),
        ),
        (
            "create",
            Endpoint::Create,
            format!("Create{row}Request"),
            row.clone(),
        ),
        (
            "update",
            Endpoint::Update,
            format!("Update{row}Request"),
            row.clone(),
        ),
        (
            "delete",
            Endpoint::Remove,
            format!("Delete{row}Request"),
            format!("Delete{row}Response"),
        ),
    ]
    .into_iter()
    .filter(|(_, endpoint, _, _)| feature.serves(*endpoint))
    .collect()
}

/// A message's Rust name, as prost writes it.
fn message(name: &str) -> String {
    name.to_upper_camel_case()
}

/// The module tonic writes a service's server in: `notes_service_server`.
fn server_module(feature: &Feature<'_>) -> String {
    format!("{}_server", feature.service().to_snake_case())
}

/// Renders the feature's `grpc.rs`.
pub fn render(feature: &Feature<'_>) -> String {
    let api = feature.service_api();
    let rpc = format!("{}Rpc", feature.module().to_upper_camel_case());
    let row = message(feature.row());
    let mut uses = Uses::default();
    uses.add("super::super::proto");
    uses.add(&format!("super::model::{}", feature.row()));
    uses.add(&format!("super::service::{}", feature.service()));
    uses.add(&format!("super::service_api::{api}"));
    uses.add("tonic::Status");
    uses.add("crate::shared::grpc::write");

    let mut body = String::new();
    for (name, endpoint, request, response) in rpcs(feature) {
        body.push('\n');
        render_rpc(
            &mut body,
            &mut uses,
            feature,
            (name, endpoint),
            (&request, &response),
        );
    }

    let mut output = format!(
        "{HEADER}\n//! The `{}` gRPC service: each RPC reads its request message, calls\n//! one [`{api}`] method, and answers with a message.\n//! `shared/grpc.rs` turns the values and the errors.\n\n",
        feature.table.display_name(),
    );
    // The server's trait is named like the feature's service: imported
    // under the feature's name for its RPCs.
    let server = format!(
        "use super::super::proto::{}::{} as {rpc};\n",
        server_module(feature),
        feature.service()
    );
    let rendered = uses.render();
    let (external, local) = rendered.split_once("\n\n").unwrap_or((&rendered, ""));
    output.push_str(external);
    output.push_str("\n\n");
    output.push_str(&insert_sorted(local, &server));
    output.push_str(&format!(
        "#[tonic::async_trait]\nimpl {rpc} for {} {{",
        feature.service()
    ));
    output.push_str(&body);
    output.push_str("}\n\n");
    render_row(&mut output, feature, &row);
    output
}

/// The `use` lines `local` (ending with a blank line) with `line` among
/// them, in rustfmt's order.
fn insert_sorted(local: &str, line: &str) -> String {
    // One statement per item: a long one is broken over lines.
    let mut lines: Vec<&str> = local
        .split_inclusive(";\n")
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    lines.push(line.trim_end());
    let key = |line: &&str| {
        line.trim_start_matches("use ")
            .trim_end_matches(';')
            .split("::")
            .map(|segment| (segment != "super", segment.to_owned()))
            .collect::<Vec<_>>()
    };
    lines.sort_by_key(key);
    let mut text = lines.join("\n");
    text.push_str("\n\n");
    text
}

/// One RPC: its signature, the scope read from the request's extensions,
/// the request message read into the service method's arguments, the call
/// and the answer.
fn render_rpc(
    output: &mut String,
    uses: &mut Uses,
    feature: &Feature<'_>,
    (name, endpoint): (&str, Endpoint),
    (request, response): (&str, &str),
) {
    uses.add("tonic::Request");
    uses.add("tonic::Response");
    push_signature(
        output,
        4,
        &format!("async fn {name}"),
        &[
            "&self".to_owned(),
            format!("request: Request<proto::{request}>"),
        ],
        &format!("Result<Response<proto::{response}>, Status>"),
    );
    // The trait's method is named like the RPC: called by its path.
    let mut args = vec!["self".to_owned()];
    args.extend(scope(output, uses, feature, endpoint));
    output.push_str("        let message = request.into_inner();\n");
    if endpoint.takes_key() {
        args.push(read_key(output, uses, feature));
    }
    let api = feature.service_api();
    let row = message(feature.row());
    match endpoint {
        Endpoint::List => {
            let query = feature.list_query();
            uses.add(&format!("super::dto::{query}"));
            output.push_str(&format!(
                "        let query = {query} {{\n            page: message.page,\n            per_page: message.per_page,\n            ..{query}::default()\n        }};\n"
            ));
            args.push("query".to_owned());
            push_call(
                output,
                8,
                "let page = ",
                &format!("{api}::list"),
                &args,
                ".await?;",
            );
            output.push_str(&format!(
                "        let items = page.items.into_iter().map(proto::{row}::try_from);\n"
            ));
            output.push_str(&format!(
                "        Ok(Response::new(proto::{response} {{\n            items: items.collect::<Result<_, _>>()?,\n            total: page.total,\n            page: page.page,\n            per_page: page.per_page,\n        }}))\n"
            ));
        }
        Endpoint::GetOne => {
            let callee = format!("{api}::get_one");
            push_call(output, 8, "let row = ", &callee, &args, ".await?;");
            push_row_answer(output, &row);
        }
        Endpoint::Remove => {
            push_call(output, 8, "", &format!("{api}::remove"), &args, ".await?;");
            output.push_str(&format!(
                "        Ok(Response::new(proto::{response} {{}}))\n"
            ));
        }
        _ => {
            let (dto, columns, change) = if endpoint == Endpoint::Create {
                (feature.create_dto(), create_columns(feature), false)
            } else {
                if let Some(version) = feature.version {
                    let field = version.rust_name.trim_start_matches("r#");
                    uses.add("crate::shared::grpc::read");
                    output.push_str(&format!(
                        "        let version = read(message.{}, \"{field}\")?;\n",
                        version.rust_name
                    ));
                    args.push("version".to_owned());
                }
                (feature.update_dto(), update_columns(feature), true)
            };
            uses.add(&format!("super::dto::{dto}"));
            uses.add("crate::shared::grpc::validated");
            let fields: Vec<String> = columns
                .iter()
                .map(|column| {
                    // An update's nullable column is changed by a value and
                    // kept without one: the message can't say `NULL`.
                    let list = column.base_type() == crate::schema::BaseType::Array;
                    let reader = if change && column.nullable && !list {
                        "read_change"
                    } else {
                        "read"
                    };
                    uses.add(&format!("crate::shared::grpc::{reader}"));
                    field_read(column, reader, 12)
                })
                .collect();
            if fields.is_empty() {
                output.push_str(&format!("        let input = validated({dto} {{}})?;\n"));
            } else {
                output.push_str(&format!("        let input = validated({dto} {{\n"));
                for field in fields {
                    output.push_str(&field);
                }
                output.push_str("        })?;\n");
            }
            args.push("input".to_owned());
            let method = if endpoint == Endpoint::Create {
                "create"
            } else {
                "update"
            };
            let callee = format!("{api}::{method}");
            push_call(output, 8, "let row = ", &callee, &args, ".await?;");
            push_row_answer(output, &row);
        }
    }
    output.push_str("    }\n");
}

/// The answer of a row: its message.
fn push_row_answer(output: &mut String, row: &str) {
    output.push_str(&format!(
        "        Ok(Response::new(proto::{row}::try_from(row)?))\n"
    ));
}

/// Reads who the RPC acts for from the request's extensions, where the
/// interceptor puts them, and returns the service method's leading
/// arguments: the tenant, then the caller when the method takes it.
fn scope(
    output: &mut String,
    uses: &mut Uses,
    feature: &Feature<'_>,
    endpoint: Endpoint,
) -> Vec<String> {
    let mut args = Vec::new();
    // The caller first, as the HTTP extractors check it: a call without
    // credentials is `UNAUTHENTICATED`, not a missing tenant.
    if let Some(extraction) = feature.caller_of(endpoint).extraction() {
        let reader = if feature.access == ApiAccess::Admin {
            "admin"
        } else {
            "caller"
        };
        uses.add(&format!("crate::shared::grpc::{reader}"));
        match extraction.arg {
            Some(arg) => {
                output.push_str(&format!(
                    "        let {} = {reader}(&request)?;\n",
                    extraction.binding
                ));
                args.push(arg.to_owned());
            }
            None => output.push_str(&format!("        {reader}(&request)?;\n")),
        }
    }
    if feature.tenant_of(endpoint).is_some() {
        uses.add("crate::shared::grpc::tenant");
        output.push_str("        let tenant = tenant(&request)?;\n");
        args.insert(0, "tenant".to_owned());
    }
    args
}

/// Reads the row's key from the message: `id` for one key column, `key`
/// for the key struct.
fn read_key(output: &mut String, uses: &mut Uses, feature: &Feature<'_>) -> String {
    uses.add("crate::shared::grpc::read");
    match &feature.key {
        Some(Key::Composite(columns)) => {
            let key = feature.key_type().unwrap_or_default();
            uses.add(&format!("super::model::{key}"));
            output.push_str(&format!("        let key = {key} {{\n"));
            for column in columns {
                output.push_str(&field_read(column, "read", 12));
            }
            output.push_str("        };\n");
            "key".to_owned()
        }
        Some(Key::Single(column)) => {
            let field = column.rust_name.trim_start_matches("r#");
            output.push_str(&format!(
                "        let id = read(message.{}, \"{field}\")?;\n",
                column.rust_name
            ));
            "id".to_owned()
        }
        None => String::new(),
    }
}

/// A struct literal's field read from the message's field of the same
/// name with `reader`, at `indent`: on one line when it fits, else the
/// call's arguments one per line, as rustfmt breaks it.
fn field_read(column: &Column, reader: &str, indent: usize) -> String {
    let pad = " ".repeat(indent);
    let name = &column.rust_name;
    let field = name.trim_start_matches("r#");
    let line = format!("{pad}{name}: {reader}(message.{name}, \"{field}\")?,\n");
    if line.len() - 1 <= MAX_WIDTH {
        return line;
    }
    format!("{pad}{name}: {reader}(\n{pad}    message.{name},\n{pad}    \"{field}\",\n{pad})?,\n")
}

/// `impl TryFrom<Row> for proto::Row`: the row as its message, every
/// column but the hidden ones.
fn render_row(output: &mut String, feature: &Feature<'_>, row: &str) {
    push_doc(
        output,
        0,
        &[&format!(
            "A `{}` row as its message: every column but the hidden ones.",
            feature.table.display_name()
        )],
    );
    output.push_str(&format!(
        "impl TryFrom<{}> for proto::{row} {{\n    type Error = Status;\n\n",
        feature.row()
    ));
    push_signature(
        output,
        4,
        "fn try_from",
        &[format!("row: {}", feature.row())],
        "Result<Self, Status>",
    );
    let columns = feature.response_columns();
    if columns.is_empty() {
        output.push_str(&format!("        Ok(proto::{row} {{}})\n"));
    } else {
        output.push_str(&format!("        Ok(proto::{row} {{\n"));
        for column in columns {
            let name = &column.rust_name;
            let line = format!("            {name}: write(row.{name})?,\n");
            if line.len() - 1 <= MAX_WIDTH {
                output.push_str(&line);
            } else {
                output.push_str(&format!(
                    "            {name}: write(\n                row.{name},\n            )?,\n"
                ));
            }
        }
        output.push_str("        })\n");
    }
    output.push_str("    }\n}\n");
}

/// The root's `generated.rs` under gRPC: the messages and services
/// `build.rs` compiles from `proto/`, every feature, declared, and
/// [`routes`]: every feature's service, mounted.
pub fn generated(features: &[Feature<'_>], package: &str) -> String {
    let mut output = format!(
        "{HEADER}
//! Every generated API feature, declared from its own folder, the messages
//! and services `build.rs` compiles from `proto/`, and the function `app.rs`
//! calls: [`routes`].

use sqlx::PgPool;
use tonic::service::Routes;
"
    );
    let mut servers: Vec<String> = features
        .iter()
        .map(|feature| {
            format!(
                "use self::proto::{}::{}Server;\n",
                server_module(feature),
                feature.service()
            )
        })
        .collect();
    servers.sort();
    if !servers.is_empty() {
        output.push('\n');
        output.push_str(&servers.concat());
    }
    output.push_str(&format!(
        "
/// The messages and services of `proto/`, as `build.rs` compiles them: the
/// package `{package}`.
#[allow(missing_docs, clippy::all, clippy::pedantic)]
pub mod proto {{
    tonic::include_proto!(\"{package}\");
}}
"
    ));
    output.push_str(&super::root::modules(features));
    output.push_str(&super::root::related(features));
    output.push_str(&routes(features));
    output
}

/// `routes`: every feature's service over `pool`, each in its server.
fn routes(features: &[Feature<'_>]) -> String {
    let pool = if features.is_empty() { "_pool" } else { "pool" };
    let mut output = format!(
        "\n/// Every feature's gRPC service, each over `pool`, for the server to\n/// serve.\n#[must_use]\npub fn routes({pool}: &PgPool) -> Routes {{\n"
    );
    let mut calls = Vec::new();
    for feature in features {
        let module = feature.module();
        let service = feature.service();
        push_call(
            &mut output,
            4,
            &format!("let {module} = "),
            &format!("{module}::{service}::new"),
            &["pool.clone()".to_owned()],
            ";",
        );
        calls.push(format!(".add_service({service}Server::new({module}))"));
    }
    let chain = format!("Routes::default(){}", calls.concat());
    if chain.len() <= CHAIN_WIDTH {
        output.push_str(&format!("    {chain}\n"));
    } else {
        output.push_str("    Routes::default()\n");
        for call in calls {
            output.push_str(&format!("        {call}\n"));
        }
    }
    output.push_str("}\n");
    output
}
