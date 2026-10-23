/// One page of results without a total, returned by
/// [`Select::simple_paginate`](crate::Select::simple_paginate): the rows,
/// the page's numbers, and whether another page follows — enough for
/// "previous" / "next" links, at the cost of one query and no `COUNT(*)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimplePage<T> {
    /// Records on this page.
    pub items: Vec<T>,
    /// Current page number (1-based).
    pub page: u64,
    /// Maximum number of records per page.
    pub per_page: u64,
    /// `true` if a later page exists.
    pub has_more: bool,
}

impl<T> SimplePage<T> {
    /// Assemble a page.
    pub fn new(items: Vec<T>, page: u64, per_page: u64, has_more: bool) -> Self {
        Self {
            items,
            page,
            per_page,
            has_more,
        }
    }

    /// `true` if a later page exists — Laravel's `hasMorePages`.
    pub fn has_more(&self) -> bool {
        self.has_more
    }

    /// `true` on page 1 — Laravel's `onFirstPage`.
    pub fn on_first_page(&self) -> bool {
        self.page <= 1
    }

    /// `true` if this page holds no records.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Number of records on this page.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Iterate over the records on this page.
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.items.iter()
    }

    /// Transform every record, keeping the page's numbers (e.g. into DTOs).
    pub fn map<U>(self, f: impl FnMut(T) -> U) -> SimplePage<U> {
        SimplePage {
            items: self.items.into_iter().map(f).collect(),
            page: self.page,
            per_page: self.per_page,
            has_more: self.has_more,
        }
    }

    /// The rows as a [`ModelCollection`](crate::ModelCollection), dropping
    /// the page's numbers.
    pub fn into_collection(self) -> crate::ModelCollection<T> {
        crate::ModelCollection::new(self.items)
    }
}

impl<T> IntoIterator for SimplePage<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a SimplePage<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}
