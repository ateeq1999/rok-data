//! What a cursor-paged list (`[api] cursor`) adds to the shared tools: its
//! query string, `CursorListQuery` or `CursorQuery`, appended to
//! `pagination.rs`, and its page in each `[api] pagination` shape,
//! appended to `response.rs`. The shapes match `rok_data::CursorPage`'s
//! wire shapes field for field (`rok_data::wire`).

use crate::ApiPagination;

/// The items `pagination.rs` gains: the cursor queries and the mapping of
/// a cursor that doesn't fit the list. Written without utoipa; the
/// OpenAPI pass describes them with the other list queries.
pub(super) const QUERIES: &str = r#"
/// A cursor-paged list endpoint's query string. `S` is what `sort` takes:
/// a table's generated sort key, or any text by default.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CursorListQuery<S = String> {
    /// Where the page starts: a `next_cursor` or `prev_cursor` a page of
    /// this list gave out. The first page without one.
    pub cursor: Option<String>,
    /// The rows a page holds, 1 to 100. Defaults to 20.
    pub per_page: Option<u32>,
    /// The text to search for.
    pub q: Option<String>,
    /// What to sort by.
    pub sort: Option<S>,
    /// The sort's direction. Defaults to `desc`.
    pub order: Option<SortOrder>,
}

impl<S> Default for CursorListQuery<S> {
    fn default() -> Self {
        CursorListQuery {
            cursor: None,
            per_page: None,
            q: None,
            sort: None,
            order: None,
        }
    }
}

impl<S> CursorListQuery<S> {
    /// The cursor the page starts at, `None` for the first page.
    ///
    /// # Errors
    ///
    /// A `400` when `cursor` is not a cursor.
    pub fn cursor(&self) -> Result<Option<Cursor>, AppError> {
        parse_cursor(self.cursor.as_deref())
    }

    /// The rows a page holds, with the default and limits applied.
    #[must_use]
    pub fn per_page(&self) -> u64 {
        u64::from(Pagination::new(None, self.per_page).per_page)
    }

    /// The sort's direction, `desc` unless `order` says otherwise.
    #[must_use]
    pub fn sort_order(&self) -> SortOrder {
        self.order.unwrap_or_default()
    }
}

/// The query string of a cursor-paged list whose table has no search
/// columns: a `CursorListQuery` without `q`. `S` is what `sort` takes.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CursorQuery<S = String> {
    /// Where the page starts: a `next_cursor` or `prev_cursor` a page of
    /// this list gave out. The first page without one.
    pub cursor: Option<String>,
    /// The rows a page holds, 1 to 100. Defaults to 20.
    pub per_page: Option<u32>,
    /// What to sort by.
    pub sort: Option<S>,
    /// The sort's direction. Defaults to `desc`.
    pub order: Option<SortOrder>,
}

impl<S> Default for CursorQuery<S> {
    fn default() -> Self {
        CursorQuery {
            cursor: None,
            per_page: None,
            sort: None,
            order: None,
        }
    }
}

impl<S> CursorQuery<S> {
    /// The cursor the page starts at, `None` for the first page.
    ///
    /// # Errors
    ///
    /// A `400` when `cursor` is not a cursor.
    pub fn cursor(&self) -> Result<Option<Cursor>, AppError> {
        parse_cursor(self.cursor.as_deref())
    }

    /// The rows a page holds, with the default and limits applied.
    #[must_use]
    pub fn per_page(&self) -> u64 {
        u64::from(Pagination::new(None, self.per_page).per_page)
    }

    /// The sort's direction, `desc` unless `order` says otherwise.
    #[must_use]
    pub fn sort_order(&self) -> SortOrder {
        self.order.unwrap_or_default()
    }
}

/// `cursor` parsed, a `400` when it is not a cursor.
fn parse_cursor(cursor: Option<&str>) -> Result<Option<Cursor>, AppError> {
    cursor.map(str::parse).transpose().map_err(|_| bad_cursor())
}

/// A cursor list's error: a cursor that doesn't fit the list — the wrong
/// number of values, or a value of another type than its column's — is a
/// `400`; anything else maps as usual.
#[must_use]
pub fn cursor_error(error: rok_data::Error) -> AppError {
    let mismatched = match &error {
        rok_data::Error::InvalidCursor(_) => true,
        // `operator does not exist`: a cursor value compared with a column
        // of another type.
        rok_data::Error::Database(sqlx::Error::Database(database)) => {
            database.code().as_deref() == Some("42883")
        }
        _ => false,
    };
    if mismatched {
        bad_cursor()
    } else {
        error.into()
    }
}

/// The `400` of a cursor no page of the list gave out.
fn bad_cursor() -> AppError {
    AppError::BadRequest("`cursor` is not a cursor of this list".to_owned())
}
"#;

/// The items `response.rs` gains: the compact cursor page, and the page in
/// the `pagination` shape when it is another, deriving `utoipa::ToSchema`
/// with `openapi`.
pub(super) fn shapes(pagination: ApiPagination, openapi: bool) -> String {
    let derive = if openapi {
        "#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]"
    } else {
        "#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]"
    };
    let shape = match pagination {
        ApiPagination::Paginator => PAGINATOR,
        ApiPagination::Resource => RESOURCE,
        _ => "",
    };
    format!("{COMPACT}{shape}").replace("#[derive(…)]", derive)
}

const COMPACT: &str = r#"
/// One page of a cursor-paged list: `{"items", "next", "prev", "per_page"}`,
/// `next` and `prev` the cursors of the pages on either side.
#[derive(…)]
pub struct CursorResponse<T> {
    /// The page's rows.
    pub items: Vec<T>,
    /// The next page's cursor; `null` on the last page.
    pub next: Option<String>,
    /// The previous page's cursor; `null` on the first page.
    pub prev: Option<String>,
    /// The most rows a page holds.
    pub per_page: u32,
}

impl<T> CursorResponse<T> {
    /// The same page with each item turned into another, such as a row
    /// into its response.
    #[must_use]
    pub fn map<U>(self, f: impl FnMut(T) -> U) -> CursorResponse<U> {
        CursorResponse {
            items: self.items.into_iter().map(f).collect(),
            next: self.next,
            prev: self.prev,
            per_page: self.per_page,
        }
    }
}

/// A runtime cursor page on the wire, its cursors as their tokens.
impl<T> From<rok_data::CursorPage<T>> for CursorResponse<T> {
    fn from(page: rok_data::CursorPage<T>) -> Self {
        CursorResponse {
            items: page.items,
            next: page.next.map(|cursor| cursor.to_string()),
            prev: page.prev.map(|cursor| cursor.to_string()),
            per_page: u32::try_from(page.per_page).unwrap_or(u32::MAX),
        }
    }
}
"#;

const PAGINATOR: &str = r#"
/// One page of a cursor-paged list as the paginator object: the rows in
/// `data`, and the cursor and URL of each neighbouring page.
#[derive(…)]
pub struct CursorPaginatorResponse<T> {
    /// The page's rows.
    pub data: Vec<T>,
    /// The request's path.
    pub path: String,
    /// The most rows a page holds.
    pub per_page: u32,
    /// The next page's cursor; `null` on the last page.
    pub next_cursor: Option<String>,
    /// The next page's URL; `null` on the last page.
    pub next_page_url: Option<String>,
    /// The previous page's cursor; `null` on the first page.
    pub prev_cursor: Option<String>,
    /// The previous page's URL; `null` on the first page.
    pub prev_page_url: Option<String>,
}

impl<T> CursorPaginatorResponse<T> {
    /// `page` as the paginator object, its links built from the request's
    /// `uri`: every other query parameter is kept.
    #[must_use]
    pub fn new(page: CursorResponse<T>, uri: &str) -> Self {
        let links = rok_data::wire::Links::from_request(uri);
        CursorPaginatorResponse {
            data: page.items,
            path: links.path().to_owned(),
            per_page: page.per_page,
            next_page_url: page.next.as_deref().map(|cursor| links.cursor(cursor)),
            next_cursor: page.next,
            prev_page_url: page.prev.as_deref().map(|cursor| links.cursor(cursor)),
            prev_cursor: page.prev,
        }
    }
}
"#;

const RESOURCE: &str = r#"
/// One page of a cursor-paged list in the resource envelope:
/// `{data, links, meta}`.
#[derive(…)]
pub struct CursorResourceResponse<T> {
    /// The page's rows.
    pub data: Vec<T>,
    /// The URLs of the neighbouring pages; a cursor page has no `first` or
    /// `last`.
    pub links: PageLinks,
    /// The page's cursors.
    pub meta: CursorPageMeta,
}

/// The `meta` of a cursor page in the resource envelope.
#[derive(…)]
pub struct CursorPageMeta {
    /// The request's path.
    pub path: String,
    /// The most rows a page holds.
    pub per_page: u32,
    /// The next page's cursor; `null` on the last page.
    pub next_cursor: Option<String>,
    /// The previous page's cursor; `null` on the first page.
    pub prev_cursor: Option<String>,
}

impl<T> CursorResourceResponse<T> {
    /// `page` in the resource envelope, its links built from the request's
    /// `uri`: every other query parameter is kept.
    #[must_use]
    pub fn new(page: CursorResponse<T>, uri: &str) -> Self {
        let links = rok_data::wire::Links::from_request(uri);
        CursorResourceResponse {
            links: PageLinks {
                first: None,
                last: None,
                prev: page.prev.as_deref().map(|cursor| links.cursor(cursor)),
                next: page.next.as_deref().map(|cursor| links.cursor(cursor)),
            },
            meta: CursorPageMeta {
                path: links.path().to_owned(),
                per_page: page.per_page,
                next_cursor: page.next,
                prev_cursor: page.prev,
            },
            data: page.items,
        }
    }
}
"#;
