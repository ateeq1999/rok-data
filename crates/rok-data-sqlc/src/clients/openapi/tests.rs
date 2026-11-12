use serde_json::{Value, json};

use super::*;

/// The monorepo template's api App's exported document, as the template
/// ships it.
fn monorepo_document() -> String {
    rok_templates::catalog::files("monorepo")
        .and_then(|files| {
            files
                .iter()
                .find(|file| file.path == "apps/api/openapi.json")
        })
        .map(|file| String::from_utf8_lossy(file.bytes).into_owned())
        .unwrap_or_else(|| panic!("the monorepo template ships no apps/api/openapi.json"))
}

/// A minimal 3.1 document holding `paths` and `components`.
fn document(paths: Value, components: Value) -> String {
    json!({
        "openapi": "3.1.0",
        "info": {"title": "t", "version": "1"},
        "paths": paths,
        "components": components,
    })
    .to_string()
}

/// A document with one `GET /things` operation, whose object is `extra`
/// merged over a `200` JSON response of `{"type": "string"}`.
fn one_operation(extra: Value) -> String {
    let mut operation = json!({
        "operationId": "list",
        "tags": ["things"],
        "responses": {"200": {"description": "ok", "content": {
            "application/json": {"schema": {"type": "string"}}
        }}},
    });
    if let (Some(base), Some(extra)) = (operation.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            base.insert(key.clone(), value.clone());
        }
    }
    document(json!({"/things": {"get": operation}}), json!({}))
}

/// The document generated features serve (`tests/api-app`).
fn api_app_document() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/api-app/tests/expected/openapi.json"
    );
    std::fs::read_to_string(path).unwrap_or_else(|err| panic!("{path}: {err}"))
}

fn parse_err(json: &str) -> String {
    match Document::parse(json) {
        Ok(document) => panic!("parsed, but should not have: {document:?}"),
        Err(err) => err.to_string(),
    }
}

fn operation<'a>(document: &'a Document, method: Method, path: &str) -> &'a Operation {
    document
        .operations
        .iter()
        .find(|operation| operation.method == method && operation.path == path)
        .unwrap_or_else(|| panic!("no {method:?} {path}"))
}

/// The document generated features serve (`tests/api-app`, whose own test
/// keeps this snapshot equal to the served one) reads whole: its enums,
/// `gadget_mood` in a body and a search filter, the sort keys and
/// `?include=` names in the query, are all defined, and a versioned row's
/// `PATCH` takes its `If-Match` header.
#[test]
fn the_generated_features_document_defines_every_ref() {
    let document = Document::parse(&api_app_document()).unwrap_or_else(|err| panic!("{err}"));
    for schema in [
        "GadgetMood",
        "Equality_GadgetMood",
        "SortOrder",
        "TaskInclude",
    ] {
        assert!(document.schemas.contains_key(schema), "{schema}");
    }
    let update = operation(&document, Method::Patch, "/api/v1/documents/{id}");
    assert_eq!(
        update.header_params,
        [Parameter {
            name: "If-Match".to_owned(),
            schema: Schema::String(Rules::default()),
            required: true,
            description: Some("The row's `ETag`.".to_owned()),
        }]
    );
    assert!(update.query_params.is_empty());
}

#[test]
fn the_monorepo_api_document_reads_whole() {
    let document = Document::parse(&monorepo_document()).unwrap_or_else(|err| panic!("{err}"));
    assert!(
        document.operations.len() > 40,
        "{}",
        document.operations.len()
    );

    let login = operation(&document, Method::Post, "/api/v1/auth/login");
    assert_eq!(login.id, "login");
    assert_eq!(login.tag.as_deref(), Some("auth"));
    assert!(!login.secured, "signing in needs no token");
    assert_eq!(
        login.body,
        Some(Body::Json(Schema::Ref("CredentialsDto".to_owned())))
    );
    assert_eq!(
        login.response,
        Response::Json(Schema::Ref("LoginResponse".to_owned()))
    );

    let list = operation(&document, Method::Get, "/api/v1/users");
    assert!(list.secured);
    let names: Vec<&str> = list.query_params.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["page", "per_page", "q", "sort", "order"]);
    assert_eq!(
        list.query_params[3].schema,
        Schema::Ref("UserSort".to_owned())
    );

    let remove = operation(&document, Method::Delete, "/api/v1/users/{id}");
    assert_eq!(remove.path_params.len(), 1);
    assert_eq!(remove.response, Response::Empty);

    let upload = operation(&document, Method::Post, "/api/v1/media");
    assert_eq!(upload.body, Some(Body::Binary));
    let avatar = operation(&document, Method::Get, "/api/v1/avatars/{file}");
    assert_eq!(avatar.response, Response::Binary);

    // `Option<Enum>` is a `oneOf` with `null`; a JSON map is an object of
    // `additionalProperties`; `allOf` composes.
    let Some(Schema::Object { properties, .. }) = document.schemas.get("CreateApiKeyDto") else {
        panic!("CreateApiKeyDto is not an object");
    };
    let scope = properties.iter().find(|p| p.name == "scope");
    assert_eq!(
        scope.map(|p| &p.schema),
        Some(&Schema::OneOf(vec![
            Schema::Null,
            Schema::Ref("ApiKeyScope".to_owned())
        ]))
    );
    assert!(matches!(
        document.schemas.get("CreatedApiKeyResponse"),
        Some(Schema::AllOf(_))
    ));
    assert_eq!(
        document.schemas.get("Role"),
        Some(&Schema::StringEnum(vec![
            "user".to_owned(),
            "admin".to_owned()
        ]))
    );
}

#[test]
fn nullable_types_maps_and_any_values_read() -> Result<()> {
    let json = document(
        json!({}),
        json!({"schemas": {"Thing": {
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": {"type": "string", "format": "uuid"},
                "note": {"type": ["string", "null"], "maxLength": 10},
                "headers": {"type": "object", "additionalProperties": {"type": "string"},
                            "propertyNames": {"type": "string"}},
                "details": {"description": "anything"},
            },
        }}}),
    );
    let document = Document::parse(&json)?;
    let Some(Schema::Object { properties, .. }) = document.schemas.get("Thing") else {
        panic!("not an object");
    };
    let by_name = |name: &str| {
        properties
            .iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("no {name}"))
    };
    assert!(by_name("id").required);
    assert!(!by_name("note").required);
    assert_eq!(
        by_name("note").schema,
        Schema::Nullable(Box::new(Schema::String(Rules {
            max_length: Some(10),
            ..Rules::default()
        })))
    );
    assert_eq!(
        by_name("headers").schema,
        Schema::Object {
            properties: Vec::new(),
            additional: Some(Box::new(Schema::String(Rules::default()))),
        }
    );
    assert_eq!(by_name("details").schema, Schema::Any);
    Ok(())
}

/// Each construct outside the subset fails the read, naming itself and
/// where it is.
#[test]
fn unsupported_constructs_are_named_with_their_place() {
    let cases = [
        (
            one_operation(json!({"parameters": [
                {"name": "session", "in": "cookie", "schema": {"type": "string"}}
            ]})),
            "#/paths/~1things/get/parameters/0: a parameter in `cookie` (path, query and header are read)",
        ),
        (
            one_operation(json!({"requestBody": {"content": {
                "multipart/form-data": {"schema": {"type": "object"}}
            }}})),
            "#/paths/~1things/get/requestBody/content/multipart~1form-data: a \
             `multipart/form-data` request body (JSON and raw bytes are read)",
        ),
        (
            one_operation(json!({"callbacks": {}})),
            "#/paths/~1things/get/callbacks: `callbacks` in an operation",
        ),
        (
            one_operation(
                json!({"responses": {"200": {"description": "ok", "content": {
                    "application/json": {"schema": {"anyOf": [{"type": "string"}]}}
                }}}}),
            ),
            "#/paths/~1things/get/responses/200/content/application~1json/schema/anyOf: \
             `anyOf` in a schema",
        ),
        (
            one_operation(
                json!({"responses": {"200": {"description": "ok", "content": {
                    "application/json": {"schema": {"$ref": "#/components/responses/Thing"}}
                }}}}),
            ),
            "#/paths/~1things/get/responses/200/content/application~1json/schema/$ref: a \
             `$ref` to `#/components/responses/Thing` (only `#/components/schemas/<name>` is read)",
        ),
        (
            document(
                json!({}),
                json!({"securitySchemes": {"key": {"type": "apiKey", "in": "header", "name": "X"}}}),
            ),
            "#/components/securitySchemes/key: a security scheme of type `apiKey` (an HTTP \
             bearer scheme is read)",
        ),
        (
            document(json!({}), json!({"responses": {}})),
            "#/components/responses: `responses` in the components",
        ),
        (
            json!({"openapi": "3.0.3", "info": {}, "paths": {}}).to_string(),
            "#/openapi: OpenAPI version `3.0.3` (3.1 is read)",
        ),
        (
            document(json!({"/things": {"head": {}}}), json!({})),
            "#/paths/~1things/head: `head` in a path item",
        ),
    ];
    for (json, expected) in cases {
        let err = parse_err(&json);
        assert!(
            err.starts_with(expected) && err.contains("is not supported"),
            "expected `{expected}`, got `{err}`"
        );
    }
}

/// A document of the subset that is broken says what is wrong.
#[test]
fn broken_documents_say_what_is_wrong() {
    let cases = [
        (
            one_operation(
                json!({"responses": {"200": {"description": "ok", "content": {
                    "application/json": {"schema": {"$ref": "#/components/schemas/Gone"}}
                }}}}),
            ),
            "#/paths/~1things/get: `$ref` to `Gone`, which `#/components/schemas` does not define",
        ),
        (
            one_operation(json!({"responses": {"404": {"description": "no"}}})),
            "#/paths/~1things/get/responses: the operation has no success (`2xx`) or redirect (`3xx`) response",
        ),
        (
            document(
                json!({"/things/{id}": {"get": {"operationId": "one",
                    "responses": {"204": {"description": "ok"}}}}}),
                json!({}),
            ),
            "#/paths/~1things~1{id}/get: the path names `{id}`, but no path parameter does",
        ),
        (
            one_operation(json!({"security": [{"cookie": []}]})),
            "#/paths/~1things/get/security/0: the security scheme `cookie` is not one of",
        ),
        ("not json".to_owned(), "the OpenAPI document is not JSON"),
    ];
    for (json, expected) in cases {
        let err = parse_err(&json);
        assert!(
            err.starts_with(expected),
            "expected `{expected}`, got `{err}`"
        );
    }
}

/// The document-wide `security` applies to the operations that say none;
/// an operation's own `[]` makes it public.
#[test]
fn document_security_is_the_default_and_an_empty_list_opts_out() -> Result<()> {
    let json = json!({
        "openapi": "3.1.0",
        "info": {},
        "security": [{"bearer": []}],
        "components": {"securitySchemes": {"bearer": {"type": "http", "scheme": "bearer"}}},
        "paths": {
            "/a": {"get": {"operationId": "a", "responses": {"204": {"description": ""}}}},
            "/b": {"get": {"operationId": "b", "security": [],
                           "responses": {"204": {"description": ""}}}},
        },
    })
    .to_string();
    let document = Document::parse(&json)?;
    assert!(operation(&document, Method::Get, "/a").secured);
    assert!(!operation(&document, Method::Get, "/b").secured);
    Ok(())
}

/// utoipa writes a documented `Option<SomeEnum>` field as a `oneOf` of
/// `null` and a `$ref` holding the description: a property with no
/// description of its own takes its one branch's, and its own wins.
#[test]
fn a_nullable_reference_takes_its_branch_description() -> Result<()> {
    let document = Document::parse(&document(json!({}), next_payout()))?;
    let Some(Schema::Object { properties, .. }) = document.schemas.get("NextPayout") else {
        panic!("not an object");
    };
    let description = |name: &str| {
        properties
            .iter()
            .find(|p| p.name == name)
            .and_then(|p| p.description.as_deref())
    };
    assert_eq!(description("status"), Some("Where the payout is."));
    assert_eq!(description("kind"), Some("The payout's own kind."));
    assert_eq!(description("amount"), Some("The amount, in cents."));
    Ok(())
}

/// Karibu Pass's `NextPayout`: `status` a documented `Option<PayoutStatus>`,
/// `kind` the same with a description of its own.
pub(crate) fn next_payout() -> Value {
    let status = json!({"oneOf": [
        {"type": "null"},
        {"$ref": "#/components/schemas/PayoutStatus", "description": "Where the payout is."},
    ]});
    let mut kind = status.clone();
    kind["description"] = json!("The payout's own kind.");
    json!({"schemas": {
        "PayoutStatus": {"type": "string", "enum": ["pending", "paid"]},
        "NextPayout": {
            "type": "object",
            "required": ["amount"],
            "properties": {
                "amount": {"type": "integer", "description": "The amount, in cents."},
                "status": status,
                "kind": kind,
            },
        },
    }})
}

/// A list answers one of the engine's two paging shapes, and the reader
/// tells which: page numbers (`items`, `page`, `per_page`, `total`, read
/// with `?page=`) or cursors (`items`, `next`, read with `?cursor=`). A
/// read answering anything else, and every write, pages by neither.
#[test]
fn a_list_pages_by_number_or_by_cursor() {
    let document = Document::parse(&api_app_document()).unwrap_or_else(|err| panic!("{err}"));
    let paging = |method, path| document.paging(operation(&document, method, path));
    assert_eq!(paging(Method::Get, "/api/v1/notes"), Some(Paging::Numbered));
    assert_eq!(paging(Method::Get, "/api/v1/labels"), Some(Paging::Cursor));
    assert_eq!(paging(Method::Get, "/api/v1/notes/{id}"), None);
    assert_eq!(paging(Method::Post, "/api/v1/notes"), None);
}

/// A bare array, or a page without the query parameter that moves it on,
/// is not paged: a Client has no next page to ask for.
#[test]
fn a_bare_array_or_an_unmovable_page_is_not_paged() {
    let items = json!({"type": "array", "items": {"type": "string"}});
    let bare = one_operation(
        json!({"responses": {"200": {"description": "ok", "content": {
            "application/json": {"schema": items.clone()}
        }}}}),
    );
    let bare = Document::parse(&bare).unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(bare.paging(&bare.operations[0]), None);

    let page = json!({"type": "object", "required": ["items", "page", "per_page", "total"],
        "properties": {"items": items, "page": {"type": "integer"},
            "per_page": {"type": "integer"}, "total": {"type": "integer"}}});
    let unmovable = one_operation(
        json!({"responses": {"200": {"description": "ok", "content": {
            "application/json": {"schema": page}
        }}}}),
    );
    let unmovable = Document::parse(&unmovable).unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(unmovable.paging(&unmovable.operations[0]), None);
}

/// An operation whose answers are redirects (no `2xx`, a `3xx`) is a
/// Browser route: its path and query parameters are read, and the Clients
/// build its URL instead of calling it.
#[test]
fn a_redirect_only_operation_reads_as_a_browser_route() {
    let json = document(
        json!({"/auth/oidc/{provider}/start": {"get": {
            "operationId": "oidc_start",
            "tags": ["auth"],
            "parameters": [
                {"name": "provider", "in": "path", "required": true, "schema": {"type": "string"}},
                {"name": "return_to", "in": "query", "schema": {"type": "string"}},
            ],
            "responses": {
                "302": {"description": "to the provider", "headers": {"Location": {"schema": {"type": "string"}}}},
                "404": {"description": "no such provider"},
            },
        }}}),
        json!({}),
    );
    let document = Document::parse(&json).unwrap_or_else(|err| panic!("{err}"));
    let start = operation(&document, Method::Get, "/auth/oidc/{provider}/start");
    assert_eq!(start.response, Response::Redirect);
    assert!(start.is_browser_route());
    assert_eq!(start.path_params[0].name, "provider");
    assert_eq!(start.query_params[0].name, "return_to");
}

/// An operation with a `2xx` stays a call, whatever redirects it also
/// documents; a Browser route takes no body or header, since a browser
/// following a link sends neither.
#[test]
fn a_success_keeps_a_call_and_a_browser_route_takes_only_path_and_query() {
    let document_with = |responses: Value, extra: Value| {
        let mut operation = json!({"operationId": "go", "tags": ["auth"], "responses": responses});
        if let (Some(base), Some(extra)) = (operation.as_object_mut(), extra.as_object()) {
            for (key, value) in extra {
                base.insert(key.clone(), value.clone());
            }
        }
        document(json!({"/go": {"post": operation}}), json!({}))
    };
    let both = Document::parse(&document_with(
        json!({"200": {"description": "ok"}, "303": {"description": "see other"}}),
        json!({}),
    ))
    .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(both.operations[0].response, Response::Empty);
    assert!(!both.operations[0].is_browser_route());

    let redirect = json!({"302": {"description": "away"}});
    let body =
        json!({"requestBody": {"content": {"application/json": {"schema": {"type": "string"}}}}});
    let header =
        json!({"parameters": [{"name": "X-Trace", "in": "header", "schema": {"type": "string"}}]});
    for extra in [body, header] {
        let err = parse_err(&document_with(redirect.clone(), extra));
        assert!(
            err.starts_with(
                "#/paths/~1go/post: a Browser route (every answer a redirect) takes only path \
                 and query parameters"
            ),
            "got `{err}`"
        );
    }
}

/// A document with two Browser routes (a sign-in's start and callback at
/// an OpenID provider) beside one call of the same tag, for the Clients'
/// emission tests.
pub(crate) fn browser_routes() -> Document {
    let redirect = json!({"302": {"description": "redirect"}});
    let json = document(
        json!({
            "/auth/me": {"get": {"operationId": "me", "tags": ["auth"],
                "responses": {"204": {"description": "ok"}}}},
            "/auth/oidc/{provider}/start": {"get": {
                "operationId": "oidc_start", "tags": ["auth"],
                "summary": "Starts signing in at an OpenID provider.",
                "parameters": [
                    {"name": "provider", "in": "path", "required": true, "schema": {"type": "string"}},
                    {"name": "return_to", "in": "query", "schema": {"type": "string"}},
                ],
                "responses": redirect.clone(),
            }},
            "/auth/oidc/callback": {"get": {
                "operationId": "oidc_callback", "tags": ["auth"],
                "parameters": [
                    {"name": "code", "in": "query", "required": true, "schema": {"type": "string"}},
                    {"name": "state", "in": "query", "required": true, "schema": {"type": "string"}},
                ],
                "responses": redirect,
            }},
        }),
        json!({}),
    );
    Document::parse(&json).unwrap_or_else(|err| panic!("{err}"))
}
