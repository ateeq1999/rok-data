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
        Schema::Nullable(Box::new(Schema::String))
    );
    assert_eq!(
        by_name("headers").schema,
        Schema::Object {
            properties: Vec::new(),
            additional: Some(Box::new(Schema::String)),
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
                {"name": "X-Trace", "in": "header", "schema": {"type": "string"}}
            ]})),
            "#/paths/~1things/get/parameters/0: a parameter in `header` (path and query are read)",
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
            "#/paths/~1things/get/responses: the operation has no success (`2xx`) response",
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
