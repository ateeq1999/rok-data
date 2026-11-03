//! The gRPC application under test: its generated services, served on one
//! end of an in-process pipe and called through the generated clients on
//! the other.
//!
//! The calls that stop before the data layer run over a pool that never
//! connects. The ones that reach it need `ROK_DATA_SQLC_TEST_DATABASE_URL`
//! (a throwaway PostgreSQL database: each run makes, then drops, a schema
//! of its own) and skip, with a note, without it.

use std::io;

use hyper_util::rt::TokioIo;
use sqlx::postgres::PgPoolOptions;
use sqlx::{AssertSqlSafe, PgPool};
use tonic::service::{InterceptorLayer, Routes};
use tonic::transport::{Channel, Endpoint, Server, Uri};
use tonic::{Code, Request, Status};
use tonic_types::StatusExt;
use uuid::Uuid;

use rok_data_sqlc_grpc_app::app::routes;
use rok_data_sqlc_grpc_app::features::proto;
use rok_data_sqlc_grpc_app::features::proto::accounts_service_client::AccountsServiceClient;
use rok_data_sqlc_grpc_app::features::proto::gadgets_service_client::GadgetsServiceClient;
use rok_data_sqlc_grpc_app::features::proto::labels_service_client::LabelsServiceClient;
use rok_data_sqlc_grpc_app::features::proto::notes_service_client::NotesServiceClient;
use rok_data_sqlc_grpc_app::shared::auth::{AuthUser, Role};

/// The tenant every call of the tests is scoped to.
const TENANT: Uuid = Uuid::from_u128(0x0192_0000_0000_7000_8000_0000_0000_0001);

/// The metadata naming the test's caller: `<id>` or `<id>/admin`.
const CALLER: &str = "x-test-caller";

/// The tests' stand-in for the template's interceptor: the caller named in
/// the metadata, and the tenant, put in the request's extensions where the
/// services read them.
#[allow(clippy::unnecessary_wraps, reason = "an interceptor's signature")]
fn scope(mut request: Request<()>) -> Result<Request<()>, Status> {
    let caller = request
        .metadata()
        .get(CALLER)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    if let Some(caller) = caller {
        let (id, role) = match caller.strip_suffix("/admin") {
            Some(id) => (id, Role::Admin),
            None => (caller.as_str(), Role::User),
        };
        let id = Uuid::parse_str(id).expect("a caller's id");
        request.extensions_mut().insert(AuthUser { id, role });
    }
    request.extensions_mut().insert(TENANT);
    Ok(request)
}

/// A channel to the application's `routes`, served on the other end of an
/// in-process pipe.
async fn channel(routes: Routes) -> Channel {
    let (client, server) = tokio::io::duplex(1024 * 1024);
    tokio::spawn(async move {
        Server::builder()
            .layer(InterceptorLayer::new(scope))
            .add_routes(routes)
            .serve_with_incoming(tokio_stream::once(Ok::<_, io::Error>(server)))
            .await
    });
    let mut client = Some(client);
    Endpoint::try_from("http://in-process")
        .expect("a well-formed URI")
        .connect_with_connector(tower::service_fn(move |_: Uri| {
            let client = client.take();
            async move {
                client
                    .map(TokioIo::new)
                    .ok_or_else(|| io::Error::other("the pipe connects once"))
            }
        }))
        .await
        .expect("the in-process channel connects")
}

/// A channel to the application over a pool that never connects.
async fn offline() -> Channel {
    let pool = PgPool::connect_lazy("postgres://localhost/unused").expect("a well-formed URL");
    channel(routes(&pool)).await
}

/// `message` as the caller `id`, an admin when `admin`.
fn as_caller<T>(message: T, id: Uuid, admin: bool) -> Request<T> {
    let mut request = Request::new(message);
    let caller = if admin {
        format!("{id}/admin")
    } else {
        id.to_string()
    };
    request
        .metadata_mut()
        .insert(CALLER, caller.parse().expect("an ASCII caller"));
    request
}

/// A call to a service whose rows need a caller, without one, is
/// `UNAUTHENTICATED`.
#[tokio::test]
async fn a_call_without_a_caller_is_unauthenticated() {
    let mut notes = NotesServiceClient::new(offline().await);
    let request = proto::GetNoteRequest {
        id: Uuid::now_v7().to_string(),
    };
    let status = notes.get(request).await.expect_err("no caller");
    assert_eq!(status.code(), Code::Unauthenticated, "{status:?}");
}

/// An admin-only service called by another caller is `PERMISSION_DENIED`.
#[tokio::test]
async fn another_caller_of_an_admin_service_is_permission_denied() {
    let mut accounts = AccountsServiceClient::new(offline().await);
    let request = proto::GetAccountRequest {
        id: Uuid::now_v7().to_string(),
    };
    let request = as_caller(request, Uuid::now_v7(), false);
    let status = accounts.get(request).await.expect_err("not an admin");
    assert_eq!(status.code(), Code::PermissionDenied, "{status:?}");
}

/// A field that doesn't hold a value of its column's type is
/// `INVALID_ARGUMENT`, naming the field, before the data layer is reached.
#[tokio::test]
async fn a_malformed_field_is_invalid_argument() {
    let mut notes = NotesServiceClient::new(offline().await);
    let request = proto::GetNoteRequest {
        id: "not-a-uuid".to_owned(),
    };
    let request = as_caller(request, Uuid::now_v7(), false);
    let status = notes.get(request).await.expect_err("a malformed key");
    assert_eq!(status.code(), Code::InvalidArgument, "{status:?}");
    assert_eq!(status.message(), "`id` is not a UUID");

    let mut gadgets = GadgetsServiceClient::new(offline().await);
    let mut request = gadget();
    request.small = 70_000;
    let status = gadgets.create(request).await.expect_err("out of range");
    assert_eq!(status.code(), Code::InvalidArgument, "{status:?}");
    assert_eq!(status.message(), "`small` is not a 16-bit integer");

    let mut request = gadget();
    request.made_on = None;
    let status = gadgets.create(request).await.expect_err("left out");
    assert_eq!(status.code(), Code::InvalidArgument, "{status:?}");
    assert_eq!(status.message(), "`made_on` is required");
}

/// A gadget to create: a value in every field.
fn gadget() -> proto::CreateGadgetRequest {
    let meta = prost_types::Struct {
        fields: [(
            "colour".to_owned(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue("red".to_owned())),
            },
        )]
        .into_iter()
        .collect(),
    };
    proto::CreateGadgetRequest {
        code: Uuid::now_v7().to_string(),
        small: 7,
        count: 70_000,
        big: Some(1 << 40),
        ratio: 0.5,
        score: 2.25,
        price: "19.99".to_owned(),
        active: None,
        blob: vec![0, 1, 2],
        made_on: Some(prost_types::Timestamp {
            seconds: 1_767_225_600,
            nanos: 0,
        }),
        seen_at: Some(prost_types::Timestamp {
            seconds: 1_767_225_601,
            nanos: 500_000_000,
        }),
        created_at: None,
        meta: Some(meta),
        extra: None,
        mood: proto::gadget::Mood::VerySad.into(),
        tags: vec!["new".to_owned(), "shiny".to_owned()],
        aliases: Vec::new(),
        nickname: Some("gizmo".to_owned()),
    }
}

/// A pool on a schema of its own in the test database, holding the
/// fixture's tables, and the schema's name; `None`, with a note, without
/// `ROK_DATA_SQLC_TEST_DATABASE_URL`.
async fn database() -> Option<(PgPool, String)> {
    let Ok(url) = std::env::var("ROK_DATA_SQLC_TEST_DATABASE_URL") else {
        eprintln!(
            "skipped: set ROK_DATA_SQLC_TEST_DATABASE_URL to a throwaway database to call the services through the data layer"
        );
        return None;
    };
    let schema = format!("grpc_app_{}", Uuid::now_v7().simple());
    let admin = PgPool::connect(&url).await.expect("the test database");
    // The schema is named from a UUID: no injection.
    sqlx::raw_sql(AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
        .execute(&admin)
        .await
        .expect("a schema of the test's own");
    let path = schema.clone();
    let pool = PgPoolOptions::new()
        .after_connect(move |conn, _| {
            let set = format!("SET search_path TO {path}");
            Box::pin(async move {
                sqlx::raw_sql(AssertSqlSafe(set)).execute(conn).await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .expect("the test database");
    sqlx::raw_sql(include_str!("../migrations/0001_notes.sql"))
        .execute(&pool)
        .await
        .expect("the fixture's tables");
    Some((pool, schema))
}

/// Drops the test's schema.
async fn drop_schema(pool: &PgPool, schema: &str) {
    sqlx::raw_sql(AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
        .execute(pool)
        .await
        .expect("the test's schema drops");
}

/// The unary RPCs through the data layer: create, get, list (with the
/// page's fields), update, delete; a row that isn't there is `NOT_FOUND`,
/// a value another row has `ALREADY_EXISTS`, its field in the details.
#[tokio::test]
async fn the_services_create_get_list_update_and_delete() {
    let Some((pool, schema)) = database().await else {
        return;
    };
    let channel = channel(routes(&pool)).await;
    let mut labels = LabelsServiceClient::new(channel.clone());

    let created = labels
        .create(proto::CreateLabelRequest {
            name: "urgent".to_owned(),
        })
        .await
        .expect("created")
        .into_inner();
    assert_eq!(created.name, "urgent");
    let got = labels
        .get(proto::GetLabelRequest { id: created.id })
        .await
        .expect("got")
        .into_inner();
    assert_eq!(got, created);

    labels
        .create(proto::CreateLabelRequest {
            name: "later".to_owned(),
        })
        .await
        .expect("created");
    let page = labels
        .list(proto::ListLabelsRequest {
            page: Some(1),
            per_page: Some(1),
        })
        .await
        .expect("listed")
        .into_inner();
    assert_eq!((page.total, page.page, page.per_page), (2, 1, 1));
    assert_eq!(page.items.len(), 1);

    let updated = labels
        .update(proto::UpdateLabelRequest {
            id: created.id,
            name: Some("now".to_owned()),
        })
        .await
        .expect("updated")
        .into_inner();
    assert_eq!(updated.name, "now");

    let status = labels
        .create(proto::CreateLabelRequest {
            name: "now".to_owned(),
        })
        .await
        .expect_err("taken");
    assert_eq!(status.code(), Code::AlreadyExists, "{status:?}");
    let violations = status
        .get_details_bad_request()
        .expect("the field errors")
        .field_violations;
    assert_eq!(violations[0].field, "name");

    labels
        .delete(proto::DeleteLabelRequest { id: created.id })
        .await
        .expect("deleted");
    let status = labels
        .get(proto::GetLabelRequest { id: created.id })
        .await
        .expect_err("gone");
    assert_eq!(status.code(), Code::NotFound, "{status:?}");

    drop_schema(&pool, &schema).await;
}

/// Every type a message carries comes back as it went in.
#[tokio::test]
async fn every_type_round_trips() {
    let Some((pool, schema)) = database().await else {
        return;
    };
    let mut gadgets = GadgetsServiceClient::new(channel(routes(&pool)).await);
    let request = gadget();
    let created = gadgets
        .create(request.clone())
        .await
        .expect("created")
        .into_inner();
    let got = gadgets
        .get(proto::GetGadgetRequest { id: created.id })
        .await
        .expect("got")
        .into_inner();
    assert_eq!(got, created);
    assert_eq!(
        (got.code, got.small, got.count, got.big),
        (request.code, 7, 70_000, Some(1 << 40))
    );
    assert_eq!((got.price.as_str(), got.blob), ("19.99", vec![0, 1, 2]));
    assert_eq!(
        (got.made_on, got.seen_at),
        (request.made_on, request.seen_at)
    );
    assert_eq!((got.meta, got.extra), (request.meta, None));
    assert_eq!(got.mood, i32::from(proto::gadget::Mood::VerySad));
    assert_eq!(
        (got.tags, got.aliases),
        (request.tags, Vec::<String>::new())
    );
    // The defaulted columns: the database's values.
    assert!(got.active);
    assert!(got.created_at.is_some());

    drop_schema(&pool, &schema).await;
}

/// A tenant-scoped, owned and versioned table: the caller and the tenant
/// come from the request's extensions, and an update names the version it
/// read.
#[tokio::test]
async fn owned_rows_take_the_caller_and_the_version() {
    let Some((pool, schema)) = database().await else {
        return;
    };
    let channel = channel(routes(&pool)).await;
    let admin = Uuid::now_v7();
    let mut accounts = AccountsServiceClient::new(channel.clone());
    let account = accounts
        .create(as_caller(
            proto::CreateAccountRequest {
                email: "ada@example.com".to_owned(),
                password_hash: "hash".to_owned(),
            },
            admin,
            true,
        ))
        .await
        .expect("an account")
        .into_inner();
    let owner = Uuid::parse_str(&account.id).expect("a UUID");

    let mut notes = NotesServiceClient::new(channel);
    let note = notes
        .create(as_caller(
            proto::CreateNoteRequest {
                title: "Plan".to_owned(),
                body: "Write it".to_owned(),
            },
            owner,
            false,
        ))
        .await
        .expect("a note")
        .into_inner();
    assert_eq!(
        (note.owner_id.as_str(), note.revision),
        (account.id.as_str(), 1)
    );
    assert_eq!(note.tenant_id, TENANT.to_string());

    let updated = notes
        .update(as_caller(
            proto::UpdateNoteRequest {
                id: note.id.clone(),
                title: Some("Plan B".to_owned()),
                body: None,
                revision: note.revision,
            },
            owner,
            false,
        ))
        .await
        .expect("updated")
        .into_inner();
    assert_eq!(
        (updated.title.as_str(), updated.body.as_str()),
        ("Plan B", "Write it")
    );

    let status = notes
        .update(as_caller(
            proto::UpdateNoteRequest {
                id: note.id.clone(),
                title: Some("Plan C".to_owned()),
                body: None,
                // The database bumps the version (a trigger the fixture leaves
                // out): any other is stale.
                revision: note.revision + 1,
            },
            owner,
            false,
        ))
        .await
        .expect_err("a stale version");
    assert_eq!(status.code(), Code::Aborted, "{status:?}");

    // Another caller's note is no row of theirs.
    let status = notes
        .get(as_caller(
            proto::GetNoteRequest { id: note.id },
            Uuid::now_v7(),
            false,
        ))
        .await
        .expect_err("not theirs");
    assert_eq!(status.code(), Code::NotFound, "{status:?}");

    drop_schema(&pool, &schema).await;
}
