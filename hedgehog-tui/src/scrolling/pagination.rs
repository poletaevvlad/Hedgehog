use super::DataView;
use std::{collections::VecDeque, ops::Range};

pub(crate) trait DataProvider {
    fn request(&self, range: Range<usize>);
}

pub(crate) struct PaginatedData<T> {
    page_size: usize,
    size: usize,
    load_margins: usize,
    first_page_index: usize,
    pages: VecDeque<Option<Vec<T>>>,
    data_provider: Option<Box<dyn DataProvider>>,
}

impl<T> PaginatedData<T> {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_load_margins(mut self, margins: usize) -> Self {
        self.load_margins = margins;
        self
    }

    pub(crate) fn with_page_size(mut self, page_size: usize) -> Self {
        self.page_size = page_size;
        self
    }

    pub(crate) fn clear_provider(&mut self) {
        self.data_provider = None;
    }

    pub(crate) fn set_provider(&mut self, data_provider: impl DataProvider + 'static) {
        self.data_provider = Some(Box::new(data_provider));
    }

    fn page_index(&self, index: usize) -> usize {
        index / self.page_size
    }

    fn page_item_index(&self, index: usize) -> usize {
        index % self.page_size
    }

    #[cfg(test)]
    fn pages_range(&self) -> (usize, usize) {
        (self.first_page_index, self.pages.len())
    }

    pub(crate) fn initial_range(
        &self,
        size: usize,
        viewport: Range<usize>,
    ) -> Option<Range<usize>> {
        if size == 0 || viewport.start >= size || viewport.is_empty() {
            return None;
        }
        let viewport = viewport.start..(viewport.end.min(size));
        let first_page = self.page_index(viewport.start.saturating_sub(self.load_margins));
        let last_page = self.page_index((viewport.end + self.load_margins).saturating_sub(1));
        Some((first_page * self.page_size)..((last_page + 1) * self.page_size).min(size))
    }

    fn request_pages(&self, pages: Range<usize>) {
        if pages.is_empty() {
            return;
        }
        if let Some(provider) = &self.data_provider {
            provider.request((pages.start * self.page_size)..(pages.end * self.page_size));
        }
    }

    pub(crate) fn set_initial(&mut self, size: usize, mut data: Vec<T>, range: Range<usize>) {
        self.size = size;
        self.pages.clear();

        let first_page = range.start / self.page_size;
        self.first_page_index = first_page;
        while data.len() > self.page_size {
            let page = data.drain(0..self.page_size).collect();
            self.pages.push_back(Some(page));
        }
        self.pages.push_back(Some(data));
    }

    pub(crate) fn set(&mut self, mut data: Vec<T>, range: Range<usize>) {
        let mut page = range.start / self.page_size;
        let starting_index = if page < self.first_page_index {
            let index = (self.first_page_index - page) * self.page_size;
            page = self.first_page_index;
            index
        } else {
            0
        };

        while starting_index < data.len() && page < self.first_page_index + self.pages.len() {
            let page_items = data
                .drain(starting_index..(starting_index + self.page_size).min(data.len()))
                .collect();
            self.pages[page - self.first_page_index] = Some(page_items);
            page += 1;
        }
    }

    pub(crate) fn clear(&mut self) {
        self.size = 0;
        self.first_page_index = 0;
        self.pages.clear();
    }
}

impl<T> Default for PaginatedData<T> {
    fn default() -> Self {
        Self {
            page_size: 128,
            size: 0,
            load_margins: 32,
            first_page_index: 0,
            pages: VecDeque::new(),
            data_provider: None,
        }
    }
}

impl<T> DataView for PaginatedData<T> {
    type Item = T;

    fn size(&self) -> usize {
        self.size
    }

    fn item_at(&self, index: usize) -> Option<&Self::Item> {
        let page_index = self.page_index(index);
        if page_index < self.first_page_index {
            return None;
        }
        self.pages
            .get(page_index - self.first_page_index)
            .and_then(|page| page.as_ref())
            .and_then(|page| page.get(self.page_item_index(index)))
    }

    fn find(&self, p: impl Fn(&Self::Item) -> bool) -> Option<usize> {
        for (page_index, page) in self.pages.iter().enumerate() {
            if let Some(page_items) = page {
                for (item_index, item) in page_items.iter().enumerate() {
                    if p(item) {
                        return Some(
                            item_index + (self.first_page_index + page_index) * self.page_size,
                        );
                    }
                }
            }
        }
        None
    }

    fn prepare(&mut self, range: Range<usize>) {
        let first_required_page = self.page_index(range.start.saturating_sub(self.load_margins));
        let last_required_page =
            self.page_index(((range.end + self.load_margins).saturating_sub(1)).min(self.size));
        let last_current_page = self.first_page_index + self.pages.len().saturating_sub(1);

        if self.pages.is_empty()
            || last_required_page < self.first_page_index
            || last_current_page < first_required_page
        {
            self.pages.clear();
            for _i in first_required_page..=last_required_page {
                self.pages.push_front(None);
            }
            self.request_pages(first_required_page..(last_required_page + 1));
            self.first_page_index = first_required_page;
        } else {
            while self.first_page_index < first_required_page {
                self.pages.pop_front();
                self.first_page_index += 1;
            }
            self.request_pages(first_required_page..self.first_page_index);
            while self.first_page_index > first_required_page {
                self.pages.push_front(None);
                self.first_page_index -= 1;
            }

            let indices_count = last_required_page - first_required_page + 1;
            if indices_count < self.pages.len() {
                self.pages.drain(indices_count..);
            } else {
                self.request_pages((last_current_page + 1)..(last_required_page + 1));
                while self.pages.len() < indices_count {
                    self.pages.push_back(None);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DataProvider, PaginatedData};
    use crate::scrolling::DataView;
    use std::{cell::RefCell, collections::VecDeque, ops::Range, rc::Rc};

    struct MockProvider(Rc<RefCell<VecDeque<Range<usize>>>>);

    impl DataProvider for MockProvider {
        fn request(&self, range: std::ops::Range<usize>) {
            self.0.borrow_mut().push_front(range);
        }
    }

    #[test]
    fn initial_range() {
        let list = PaginatedData::<i32>::new()
            .with_page_size(3)
            .with_load_margins(1);

        assert_eq!(list.initial_range(14, 0..4), Some(0..6));
        assert_eq!(list.initial_range(14, 1..5), Some(0..6));
        assert_eq!(list.initial_range(14, 2..6), Some(0..9));
        assert_eq!(list.initial_range(14, 4..8), Some(3..9));
        assert_eq!(list.initial_range(14, 5..9), Some(3..12));
        assert_eq!(list.initial_range(14, 10..14), Some(9..14));
        assert_eq!(list.initial_range(14, 11..15), Some(9..14));
        assert_eq!(list.initial_range(14, 13..17), Some(12..14));
        assert_eq!(list.initial_range(14, 14..18), None);
        assert_eq!(list.initial_range(0, 3..7), None);
    }

    #[test]
    fn initializing_data() {
        let mut list = PaginatedData::<usize>::new()
            .with_page_size(4)
            .with_load_margins(0);

        assert_eq!(list.size(), 0);

        list.set_initial(40, (12..20).collect(), 12..20);
        for i in 0..40 {
            if (12..20).contains(&i) {
                assert_eq!(list.item_at(i), Some(&i));
            } else {
                assert_eq!(list.item_at(i), None);
            }
        }

        assert_eq!(list.pages_range(), (3, 2));
    }

    fn list_items<T: Default + Clone>(list: &PaginatedData<T>) -> Vec<T> {
        let mut items = Vec::with_capacity(list.size());
        for index in 0..list.size {
            items.push(list.item_at(index).cloned().unwrap_or_default());
        }
        items
    }

    #[test]
    fn set_data() {
        let mut list = PaginatedData::<usize>::new().with_page_size(2);
        list.set_initial(12, vec![31, 32, 41, 42], 4..8);
        assert_eq!(
            list_items(&list),
            vec![0, 0, 0, 0, 31, 32, 41, 42, 0, 0, 0, 0]
        );

        // before first page, ignore
        list.set(vec![11, 12, 21, 22], 0..4);
        assert_eq!(
            list_items(&list),
            vec![0, 0, 0, 0, 31, 32, 41, 42, 0, 0, 0, 0]
        );

        // after last page, ignore
        list.set(vec![51, 52, 61, 62], 8..12);
        assert_eq!(
            list_items(&list),
            vec![0, 0, 0, 0, 31, 32, 41, 42, 0, 0, 0, 0]
        );

        // before first page, and inside the visible range ignore
        list.set(vec![21, 22, 131, 132], 2..6);
        assert_eq!(
            list_items(&list),
            vec![0, 0, 0, 0, 131, 132, 41, 42, 0, 0, 0, 0]
        );

        // inside the visible range and after the last page
        list.set(vec![141, 142, 151, 152], 6..10);
        assert_eq!(
            list_items(&list),
            vec![0, 0, 0, 0, 131, 132, 141, 142, 0, 0, 0, 0]
        );

        // inside and outside of the visible range
        list.set(vec![221, 222, 231, 232, 241, 242, 251, 252], 2..10);
        assert_eq!(
            list_items(&list),
            vec![0, 0, 0, 0, 231, 232, 241, 242, 0, 0, 0, 0]
        );
    }

    #[test]
    fn requesting_and_dropping_pages() {
        let mut list = PaginatedData::<usize>::new()
            .with_page_size(3)
            .with_load_margins(1);
        let requested = Rc::new(RefCell::new(VecDeque::new()));
        list.set_provider(MockProvider(requested.clone()));

        list.set_initial(12, vec![4, 5, 6, 7, 8, 9], 3..9);
        assert_eq!(requested.borrow_mut().pop_back(), None);
        assert_eq!(list.pages_range(), (1, 2));

        list.prepare(4..6);
        assert_eq!(requested.borrow_mut().pop_back(), None);
        assert_eq!(list.pages_range(), (1, 2));

        list.prepare(3..5);
        assert_eq!(requested.borrow_mut().pop_back(), Some(0..3));
        assert_eq!(requested.borrow_mut().pop_back(), None);
        assert_eq!(list.pages_range(), (0, 2));

        list.prepare(8..10);
        assert_eq!(requested.borrow_mut().pop_back(), Some(6..12));
        assert_eq!(requested.borrow_mut().pop_back(), None);
        assert_eq!(list.pages_range(), (2, 2));

        list.prepare(0..2);
        assert_eq!(requested.borrow_mut().pop_back(), Some(0..3));
        assert_eq!(requested.borrow_mut().pop_back(), None);
        assert_eq!(list.pages_range(), (0, 1));

        list.prepare(4..5);
        assert_eq!(requested.borrow_mut().pop_back(), Some(3..6));
        assert_eq!(requested.borrow_mut().pop_back(), None);
        assert_eq!(list.pages_range(), (1, 1));

        list.prepare(2..7);
        assert_eq!(requested.borrow_mut().pop_back(), Some(0..3));
        assert_eq!(requested.borrow_mut().pop_back(), Some(6..9));
        assert_eq!(requested.borrow_mut().pop_back(), None);
        assert_eq!(list.pages_range(), (0, 3));

        list.prepare(4..5);
        assert_eq!(requested.borrow_mut().pop_back(), None);
        assert_eq!(list.pages_range(), (1, 1));
    }
}
