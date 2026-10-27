/// One page of results, returned by [`Select::paginate`](crate::Select::paginate).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    /// Records on this page.
    pub items: Vec<T>,
    /// Total number of matching records across all pages.
    pub total: u64,
    /// Current page number (1-based).
    pub page: u64,
    /// Maximum number of records per page.
    pub per_page: u64,
}

impl<T> Page<T> {
    /// Assemble a page.
    pub fn new(items: Vec<T>, total: u64, page: u64, per_page: u64) -> Self {
        Self {
            items,
            total,
            page,
            per_page,
        }
    }

    /// Total number of pages (at least 1).
    pub fn total_pages(&self) -> u64 {
        if self.per_page == 0 {
            return 1;
        }
        self.total.div_ceil(self.per_page).max(1)
    }

    /// `true` if a later page exists.
    pub fn has_next(&self) -> bool {
        self.page < self.total_pages()
    }

    /// `true` if an earlier page exists.
    pub fn has_prev(&self) -> bool {
        self.page > 1
    }

    /// The number of the last page (at least 1);
    /// the same as [`total_pages`](Self::total_pages).
    pub fn last_page(&self) -> u64 {
        self.total_pages()
    }

    /// `true` on page 1.
    pub fn on_first_page(&self) -> bool {
        self.page <= 1
    }

    /// `true` on the last page, or past it.
    pub fn on_last_page(&self) -> bool {
        self.page >= self.last_page()
    }

    /// `true` when the rows span more than one page, for whether to show
    /// page links at all.
    pub fn has_pages(&self) -> bool {
        self.last_page() > 1
    }

    /// The 1-based position of this page's first row among all matches
    /// (`(page - 1) * per_page + 1`), or `None` on an empty page, for
    /// "showing 21–40 of 95".
    pub fn first_item(&self) -> Option<u64> {
        (!self.items.is_empty()).then(|| {
            (self.page - 1)
                .saturating_mul(self.per_page)
                .saturating_add(1)
        })
    }

    /// The 1-based position of this page's last row, or `None` on an empty
    /// page.
    pub fn last_item(&self) -> Option<u64> {
        self.first_item()
            .map(|first| first + self.items.len() as u64 - 1)
    }

    /// The rows as a [`ModelCollection`](crate::ModelCollection), dropping
    /// the page's numbers.
    pub fn into_collection(self) -> crate::ModelCollection<T> {
        crate::ModelCollection::new(self.items)
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

    /// Transform every record, keeping pagination metadata (e.g. into DTOs).
    pub fn map<U>(self, f: impl FnMut(T) -> U) -> Page<U> {
        Page {
            items: self.items.into_iter().map(f).collect(),
            total: self.total,
            page: self.page,
            per_page: self.per_page,
        }
    }
}

impl<T> IntoIterator for Page<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a Page<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_math() {
        let p = Page::new(vec![1, 2], 5, 1, 2);
        assert_eq!(p.total_pages(), 3);
        assert!(p.has_next() && !p.has_prev());
        let p = Page::new(Vec::<i32>::new(), 0, 1, 10);
        assert_eq!(p.total_pages(), 1);
        assert!(!p.has_next());
        assert_eq!(Page::new(vec![1], 1, 1, 1).map(|x| x * 2).items, vec![2]);
    }

    #[test]
    fn the_page_position_helpers() {
        // Page 2 of 95 rows at 20 a page: rows 21–40 of 5 pages.
        let middle = Page::new(vec![0; 20], 95, 2, 20);
        assert_eq!(middle.last_page(), 5);
        assert_eq!(
            (middle.first_item(), middle.last_item()),
            (Some(21), Some(40))
        );
        assert!(!middle.on_first_page() && !middle.on_last_page());
        assert!(middle.has_pages());

        // The last page holds the remainder.
        let last = Page::new(vec![0; 15], 95, 5, 20);
        assert_eq!((last.first_item(), last.last_item()), (Some(81), Some(95)));
        assert!(last.on_last_page() && !last.has_next());

        // A single page: no links needed.
        let only = Page::new(vec![0; 3], 3, 1, 20);
        assert!(only.on_first_page() && only.on_last_page() && !only.has_pages());

        // Past the end: empty, numbers intact, no item positions.
        let past = Page::new(Vec::<i32>::new(), 95, 9, 20);
        assert_eq!((past.first_item(), past.last_item()), (None, None));
        assert!(past.on_last_page());
        assert_eq!(past.into_collection().len(), 0);
    }
}
