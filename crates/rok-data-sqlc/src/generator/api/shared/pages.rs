//! The page shapes `[api] pagination` picks for the list endpoints,
//! appended to `shared/response.rs`: `PaginatorResponse` and
//! `ResourceResponse`, each built from the `PaginatedResponse` a service
//! returns and the request's URI. They match `rok_data::wire`'s shapes
//! field for field.

use crate::ApiPagination;

/// The items `response.rs` gains for `pagination` (none for `compact`),
/// deriving `utoipa::ToSchema` with `openapi`.
pub(super) fn shapes(pagination: ApiPagination, openapi: bool) -> String {
    let derive = if openapi {
        "#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]"
    } else {
        "#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]"
    };
    let shape = match pagination {
        ApiPagination::Paginator => PAGINATOR,
        ApiPagination::Resource => RESOURCE,
        _ => return String::new(),
    };
    format!("{shape}{NUMBERS}").replace("#[derive(…)]", derive)
}

const PAGINATOR: &str = r#"
/// One page of a list as the paginator object: the rows in `data`, the
/// page numbers, `from` / `to`, and a URL for each neighbouring page.
#[derive(…)]
pub struct PaginatorResponse<T> {
    /// The page, from 1.
    pub current_page: u32,
    /// The page's rows.
    pub data: Vec<T>,
    /// The first page's URL.
    pub first_page_url: String,
    /// The position of the page's first row, from 1; `null` on an empty page.
    pub from: Option<u64>,
    /// The last page's number.
    pub last_page: u64,
    /// The last page's URL.
    pub last_page_url: String,
    /// The next page's URL; `null` on the last page.
    pub next_page_url: Option<String>,
    /// The request's path.
    pub path: String,
    /// The most rows a page holds.
    pub per_page: u32,
    /// The previous page's URL; `null` on the first page.
    pub prev_page_url: Option<String>,
    /// The position of the page's last row, from 1; `null` on an empty page.
    pub to: Option<u64>,
    /// The rows on every page together.
    pub total: i64,
}

impl<T> PaginatorResponse<T> {
    /// `page` as the paginator object, its links built from the request's
    /// `uri`: every other query parameter is kept.
    #[must_use]
    pub fn new(page: PaginatedResponse<T>, uri: &str) -> Self {
        let links = rok_data::wire::Links::from_request(uri);
        let numbers = PageNumbers::of(&page);
        PaginatorResponse {
            current_page: page.page,
            first_page_url: links.page(1),
            from: numbers.from,
            last_page: numbers.last,
            last_page_url: links.page(numbers.last),
            next_page_url: numbers.next.map(|next| links.page(next)),
            path: links.path().to_owned(),
            per_page: page.per_page,
            prev_page_url: numbers.prev.map(|prev| links.page(prev)),
            to: numbers.to,
            total: page.total,
            data: page.items,
        }
    }
}
"#;

const RESOURCE: &str = r#"
/// One page of a list in the resource envelope: `{data, links, meta}`.
#[derive(…)]
pub struct ResourceResponse<T> {
    /// The page's rows.
    pub data: Vec<T>,
    /// The URLs of the neighbouring pages.
    pub links: PageLinks,
    /// The page's numbers.
    pub meta: PageMeta,
}

/// The `links` of a page in the resource envelope.
#[derive(…)]
pub struct PageLinks {
    /// The first page's URL.
    pub first: Option<String>,
    /// The last page's URL.
    pub last: Option<String>,
    /// The previous page's URL; `null` on the first page.
    pub prev: Option<String>,
    /// The next page's URL; `null` on the last page.
    pub next: Option<String>,
}

/// The `meta` of a page in the resource envelope.
#[derive(…)]
pub struct PageMeta {
    /// The page, from 1.
    pub current_page: u32,
    /// The position of the page's first row, from 1; `null` on an empty page.
    pub from: Option<u64>,
    /// The last page's number.
    pub last_page: u64,
    /// The request's path.
    pub path: String,
    /// The most rows a page holds.
    pub per_page: u32,
    /// The position of the page's last row, from 1; `null` on an empty page.
    pub to: Option<u64>,
    /// The rows on every page together.
    pub total: i64,
}

impl<T> ResourceResponse<T> {
    /// `page` in the resource envelope, its links built from the request's
    /// `uri`: every other query parameter is kept.
    #[must_use]
    pub fn new(page: PaginatedResponse<T>, uri: &str) -> Self {
        let links = rok_data::wire::Links::from_request(uri);
        let numbers = PageNumbers::of(&page);
        ResourceResponse {
            links: PageLinks {
                first: Some(links.page(1)),
                last: Some(links.page(numbers.last)),
                prev: numbers.prev.map(|prev| links.page(prev)),
                next: numbers.next.map(|next| links.page(next)),
            },
            meta: PageMeta {
                current_page: page.page,
                from: numbers.from,
                last_page: numbers.last,
                path: links.path().to_owned(),
                per_page: page.per_page,
                to: numbers.to,
                total: page.total,
            },
            data: page.items,
        }
    }
}
"#;

const NUMBERS: &str = r#"
/// The numbers around a page: its rows' positions and its neighbours.
struct PageNumbers {
    from: Option<u64>,
    to: Option<u64>,
    last: u64,
    prev: Option<u64>,
    next: Option<u64>,
}

impl PageNumbers {
    fn of<T>(page: &PaginatedResponse<T>) -> Self {
        let per_page = u64::from(page.per_page.max(1));
        let total = u64::try_from(page.total).unwrap_or(0);
        let current = u64::from(page.page.max(1));
        let last = total.div_ceil(per_page).max(1);
        let rows = u64::try_from(page.items.len()).unwrap_or(u64::MAX);
        let first = (current - 1).saturating_mul(per_page).saturating_add(1);
        let (from, to) = if rows == 0 {
            (None, None)
        } else {
            (Some(first), Some(first.saturating_add(rows - 1)))
        };
        PageNumbers {
            from,
            to,
            last,
            prev: (current > 1).then(|| current - 1),
            next: (current < last).then(|| current + 1),
        }
    }
}
"#;
