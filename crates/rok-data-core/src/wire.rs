//! Pagination on the wire: a page in the JSON shape an API client expects,
//! with its links built from the request ([`Links`]).
//!
//! [`Page`](crate::Page), [`SimplePage`](crate::SimplePage) and
//! [`CursorPage`](crate::CursorPage) each answer in two shapes:
//!
//! - **`paginator(links)`** — the paginator object: `data`,
//!   `current_page`, `per_page`, `total`, `last_page`, `from`, `to` and the
//!   `*_page_url`s (a simple page leaves out the total; a cursor page
//!   carries `next_cursor` / `prev_cursor`);
//! - **`resource(links)`** — the resource envelope:
//!   `{data, links: {first, last, prev, next}, meta: {…}}`.
//!
//! (The type's own `Serialize` stays the compact `{items, …}` shape.)
//!
//! [`Links`] reads the request's path and query, so every link keeps the
//! other query parameters:
//!
//! ```ignore
//! async fn books(uri: OriginalUri, State(db): State<Db>) -> rok_data::Result<Json<Value>> {
//!     let page = Book::query().paginate(&db, 2, 20).await?;
//!     Ok(Json(serde_json::to_value(page.paginator(Links::from_request(&uri.to_string())))?))
//! }
//! ```

/// Where a page's links point: the request's path and query string, the
/// name of the page parameter (`page` by default)
/// and an optional `#fragment`.
#[derive(Debug, Clone)]
pub struct Links<'a> {
    path: &'a str,
    query: Vec<&'a str>,
    page_param: &'a str,
    fragment: Option<&'a str>,
}

impl<'a> Links<'a> {
    /// The links of the request at `uri`, a path with its query string
    /// (`/api/v1/books?sort=title&page=2`). The page and cursor parameters
    /// are replaced in every link; the other parameters are kept as sent.
    pub fn from_request(uri: &'a str) -> Self {
        let (path, query) = uri.split_once('?').unwrap_or((uri, ""));
        let path = path.split_once('#').map_or(path, |(path, _)| path);
        let query = query.split_once('#').map_or(query, |(query, _)| query);
        Self {
            path,
            query: query.split('&').filter(|pair| !pair.is_empty()).collect(),
            page_param: "page",
            fragment: None,
        }
    }

    /// Names the page parameter (`?p=2` instead of `?page=2`), for two
    /// paginated lists on one page.
    pub fn page_param(mut self, name: &'a str) -> Self {
        self.page_param = name;
        self
    }

    /// Ends every link with `#fragment`.
    pub fn fragment(mut self, fragment: &'a str) -> Self {
        self.fragment = Some(fragment);
        self
    }

    /// The request's path, without the query (the shapes' `path`).
    pub fn path(&self) -> &str {
        self.path
    }

    /// The link to page `page`.
    pub fn page(&self, page: u64) -> String {
        self.with(self.page_param, &page.to_string())
    }

    /// The link that continues from `cursor` (a cursor's string form).
    pub fn cursor(&self, cursor: &str) -> String {
        self.with("cursor", cursor)
    }

    /// The request's link with `param` set to `value`: the other
    /// parameters kept in their order, the page and cursor ones dropped.
    fn with(&self, param: &str, value: &str) -> String {
        let mut url = String::from(self.path);
        url.push('?');
        for pair in &self.query {
            let key = pair.split_once('=').map_or(*pair, |(key, _)| key);
            if key != self.page_param && key != "cursor" {
                url.push_str(pair);
                url.push('&');
            }
        }
        url.push_str(param);
        url.push('=');
        url.push_str(value);
        if let Some(fragment) = self.fragment {
            url.push('#');
            url.push_str(fragment);
        }
        url
    }
}

pub use shapes::{Paginator, Resource};

/// The page shapes, serialized.
mod shapes {
    use serde::ser::SerializeStruct;
    use serde::{Serialize, Serializer};

    use super::Links;
    use crate::{CursorPage, Page, SimplePage};

    /// A page as the paginator object: what [`Page::paginator`],
    /// [`SimplePage::paginator`] and [`CursorPage::paginator`] return.
    #[derive(Debug)]
    pub struct Paginator<'a, P> {
        page: &'a P,
        links: Links<'a>,
    }

    /// A page in the resource envelope, `{data, links, meta}`: what
    /// [`Page::resource`], [`SimplePage::resource`] and
    /// [`CursorPage::resource`] return.
    #[derive(Debug)]
    pub struct Resource<'a, P> {
        page: &'a P,
        links: Links<'a>,
    }

    /// The 1-based positions of a page's first and last row, `None` when empty.
    fn span(page: u64, per_page: u64, len: usize) -> (Option<u64>, Option<u64>) {
        if len == 0 {
            return (None, None);
        }
        let from = (page.max(1) - 1).saturating_mul(per_page) + 1;
        (Some(from), Some(from + len as u64 - 1))
    }

    macro_rules! shapes {
        ($page:ident) => {
            impl<T> $page<T> {
                /// This page as the paginator object — `data`, the page numbers and
                /// a URL for each neighbouring page — its links built from
                /// `links` (see the [`wire`](crate::wire) module).
                pub fn paginator<'a>(&'a self, links: Links<'a>) -> Paginator<'a, Self> {
                    Paginator { page: self, links }
                }

                /// This page in the resource envelope, `{data, links, meta}` (see the [`wire`](crate::wire) module).
                pub fn resource<'a>(&'a self, links: Links<'a>) -> Resource<'a, Self> {
                    Resource { page: self, links }
                }
            }
        };
    }

    shapes!(Page);
    shapes!(SimplePage);
    shapes!(CursorPage);

    impl<T: Serialize> Serialize for Paginator<'_, Page<T>> {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            let (page, links) = (self.page, &self.links);
            let (from, to) = span(page.page, page.per_page, page.items.len());
            let last = page.last_page();
            let mut st = s.serialize_struct("Page", 12)?;
            st.serialize_field("current_page", &page.page)?;
            st.serialize_field("data", &page.items)?;
            st.serialize_field("first_page_url", &links.page(1))?;
            st.serialize_field("from", &from)?;
            st.serialize_field("last_page", &last)?;
            st.serialize_field("last_page_url", &links.page(last))?;
            st.serialize_field(
                "next_page_url",
                &page.has_next().then(|| links.page(page.page + 1)),
            )?;
            st.serialize_field("path", links.path())?;
            st.serialize_field("per_page", &page.per_page)?;
            st.serialize_field(
                "prev_page_url",
                &page.has_prev().then(|| links.page(page.page - 1)),
            )?;
            st.serialize_field("to", &to)?;
            st.serialize_field("total", &page.total)?;
            st.end()
        }
    }

    impl<T: Serialize> Serialize for Paginator<'_, SimplePage<T>> {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            let (page, links) = (self.page, &self.links);
            let (from, to) = span(page.page, page.per_page, page.items.len());
            let mut st = s.serialize_struct("SimplePage", 9)?;
            st.serialize_field("current_page", &page.page)?;
            st.serialize_field("data", &page.items)?;
            st.serialize_field("first_page_url", &links.page(1))?;
            st.serialize_field("from", &from)?;
            st.serialize_field(
                "next_page_url",
                &page.has_more.then(|| links.page(page.page + 1)),
            )?;
            st.serialize_field("path", links.path())?;
            st.serialize_field("per_page", &page.per_page)?;
            st.serialize_field(
                "prev_page_url",
                &(page.page > 1).then(|| links.page(page.page - 1)),
            )?;
            st.serialize_field("to", &to)?;
            st.end()
        }
    }

    impl<T: Serialize> Serialize for Paginator<'_, CursorPage<T>> {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            let (page, links) = (self.page, &self.links);
            let next = page.next.as_ref().map(ToString::to_string);
            let prev = page.prev.as_ref().map(ToString::to_string);
            let mut st = s.serialize_struct("CursorPage", 7)?;
            st.serialize_field("data", &page.items)?;
            st.serialize_field("path", links.path())?;
            st.serialize_field("per_page", &page.per_page)?;
            st.serialize_field("next_cursor", &next)?;
            st.serialize_field("next_page_url", &next.as_deref().map(|c| links.cursor(c)))?;
            st.serialize_field("prev_cursor", &prev)?;
            st.serialize_field("prev_page_url", &prev.as_deref().map(|c| links.cursor(c)))?;
            st.end()
        }
    }

    /// The `links` object of a resource envelope.
    #[derive(Serialize)]
    struct LinkSet {
        first: Option<String>,
        last: Option<String>,
        prev: Option<String>,
        next: Option<String>,
    }

    impl<T: Serialize> Serialize for Resource<'_, Page<T>> {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            #[derive(Serialize)]
            struct Meta<'a> {
                current_page: u64,
                from: Option<u64>,
                last_page: u64,
                path: &'a str,
                per_page: u64,
                to: Option<u64>,
                total: u64,
            }
            let (page, links) = (self.page, &self.links);
            let (from, to) = span(page.page, page.per_page, page.items.len());
            let last = page.last_page();
            let mut st = s.serialize_struct("Page", 3)?;
            st.serialize_field("data", &page.items)?;
            st.serialize_field(
                "links",
                &LinkSet {
                    first: Some(links.page(1)),
                    last: Some(links.page(last)),
                    prev: page.has_prev().then(|| links.page(page.page - 1)),
                    next: page.has_next().then(|| links.page(page.page + 1)),
                },
            )?;
            st.serialize_field(
                "meta",
                &Meta {
                    current_page: page.page,
                    from,
                    last_page: last,
                    path: links.path(),
                    per_page: page.per_page,
                    to,
                    total: page.total,
                },
            )?;
            st.end()
        }
    }

    impl<T: Serialize> Serialize for Resource<'_, SimplePage<T>> {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            #[derive(Serialize)]
            struct Meta<'a> {
                current_page: u64,
                from: Option<u64>,
                path: &'a str,
                per_page: u64,
                to: Option<u64>,
            }
            let (page, links) = (self.page, &self.links);
            let (from, to) = span(page.page, page.per_page, page.items.len());
            let mut st = s.serialize_struct("SimplePage", 3)?;
            st.serialize_field("data", &page.items)?;
            st.serialize_field(
                "links",
                &LinkSet {
                    first: Some(links.page(1)),
                    last: None,
                    prev: (page.page > 1).then(|| links.page(page.page - 1)),
                    next: page.has_more.then(|| links.page(page.page + 1)),
                },
            )?;
            st.serialize_field(
                "meta",
                &Meta {
                    current_page: page.page,
                    from,
                    path: links.path(),
                    per_page: page.per_page,
                    to,
                },
            )?;
            st.end()
        }
    }

    impl<T: Serialize> Serialize for Resource<'_, CursorPage<T>> {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            #[derive(Serialize)]
            struct Meta<'a> {
                path: &'a str,
                per_page: u64,
                next_cursor: Option<String>,
                prev_cursor: Option<String>,
            }
            let (page, links) = (self.page, &self.links);
            let next = page.next.as_ref().map(ToString::to_string);
            let prev = page.prev.as_ref().map(ToString::to_string);
            let mut st = s.serialize_struct("CursorPage", 3)?;
            st.serialize_field("data", &page.items)?;
            st.serialize_field(
                "links",
                &LinkSet {
                    first: None,
                    last: None,
                    prev: prev.as_deref().map(|c| links.cursor(c)),
                    next: next.as_deref().map(|c| links.cursor(c)),
                },
            )?;
            st.serialize_field(
                "meta",
                &Meta {
                    path: links.path(),
                    per_page: page.per_page,
                    next_cursor: next,
                    prev_cursor: prev,
                },
            )?;
            st.end()
        }
    }
}
