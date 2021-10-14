use std::collections::VecDeque;

#[derive(Debug)]
struct Chunk<T> {
    index: usize,
    data: Option<Vec<T>>,
}

impl<T> Chunk<T> {
    fn new(index: usize) -> Self {
        Chunk { index, data: None }
    }
}

pub(crate) trait PaginatedDataProvider {
    fn request_page(&mut self, index: usize, size: usize);
}

#[derive(Debug)]
pub(crate) struct PaginatedListOptions {
    pub(crate) chunk_size: usize,
    pub(crate) loaded_margin_size: usize,
}

#[derive(Debug)]
pub(crate) struct PaginatedList<T, P> {
    provider: P,
    size: usize,
    chunks: VecDeque<Chunk<T>>,
    offset: usize,
    window_size: usize,
    options: PaginatedListOptions,
}

impl<T, P: PaginatedDataProvider> PaginatedList<T, P> {
    const DEFAULT_OPTIONS: PaginatedListOptions = PaginatedListOptions {
        chunk_size: 128,
        loaded_margin_size: 64,
    };

    pub(crate) fn new(window_size: usize, size: usize, provider: P) -> Self {
        let mut this = PaginatedList {
            provider,
            size,
            chunks: VecDeque::new(),
            offset: 0,
            window_size,
            options: Self::DEFAULT_OPTIONS,
        };
        this.update();
        this
    }

    pub(crate) fn new_with_options(
        window_size: usize,
        size: usize,
        provider: P,
        options: PaginatedListOptions,
    ) -> Self {
        let mut this = PaginatedList {
            provider,
            size,
            chunks: VecDeque::new(),
            offset: 0,
            window_size,
            options,
        };
        this.update();
        this
    }

    fn index_to_page(&self, index: usize) -> usize {
        index / self.options.chunk_size
    }

    fn index_in_page(&self, index: usize) -> usize {
        index % self.options.chunk_size
    }

    fn update(&mut self) {
        let first_required_page =
            self.index_to_page(self.offset.saturating_sub(self.options.loaded_margin_size));
        let last_required_page = self.index_to_page(
            (self.offset + self.window_size + self.options.loaded_margin_size - 1).min(self.size),
        );

        pop_front_while(&mut self.chunks, |item| item.index < first_required_page);
        pop_back_while(&mut self.chunks, |item| item.index > last_required_page);

        if let Some(first) = self.chunks.front() {
            if first.index != 0 {
                let mut index = first.index.saturating_sub(1);
                while index >= first_required_page {
                    self.chunks.push_front(Chunk::new(index));
                    self.provider.request_page(index, self.options.chunk_size);
                    if index == 0 {
                        break;
                    }
                    index = index.saturating_sub(1);
                }
            }

            let mut index = self.chunks.back().unwrap().index + 1;
            while index <= last_required_page {
                self.chunks.push_back(Chunk::new(index));
                self.provider.request_page(index, self.options.chunk_size);
                index += 1
            }
        } else {
            for index in first_required_page..=last_required_page {
                self.chunks.push_back(Chunk::new(index));
                self.provider.request_page(index, self.options.chunk_size);
            }
        }
    }

    pub(crate) fn iter<'a>(&'a self) -> PaginatedListIterator<'a, T, P> {
        PaginatedListIterator {
            list: self,
            remaining: self.window_size.min(self.size),
            index: self.chunks.front().map(|first_index| {
                (
                    self.index_to_page(self.offset)
                        .saturating_sub(first_index.index),
                    self.index_in_page(self.offset),
                )
            }),
        }
    }

    pub(crate) fn data_available(&mut self, index: usize, data: Vec<T>) {
        if let Some(first_index) = self.chunks.front().as_ref().map(|s| s.index) {
            if index < first_index {
                return;
            }
            let index = self.chunks.get_mut(index - first_index);
            if let Some(chunk) = index {
                chunk.data = Some(data);
            }
        }
    }

    pub(crate) fn set_offset(&mut self, offset: usize) {
        if self.size <= self.window_size {
            return;
        }
        let maximum_offset = self.size - self.window_size;
        self.offset = offset.min(maximum_offset);
        self.update();
    }

    pub(crate) fn scroll(&mut self, offset: isize) {
        let new_offset = if offset > 0 {
            self.offset.saturating_add(offset as usize)
        } else {
            self.offset.saturating_sub(-offset as usize)
        };
        self.set_offset(new_offset);
    }

    #[cfg(test)]
    pub(crate) fn get_page_indices(&self) -> Vec<usize> {
        self.chunks.iter().map(|chunk| chunk.index).collect()
    }
}

#[derive(Debug)]
pub(crate) struct PaginatedListIterator<'a, T: 'a, P> {
    list: &'a PaginatedList<T, P>,
    remaining: usize,
    index: Option<(usize, usize)>,
}

impl<'a, T: 'a, P> Iterator for PaginatedListIterator<'a, T, P> {
    type Item = Option<&'a T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let result = match self.index {
            None => None,
            Some((ref mut chunk_index, ref mut item_index)) => {
                let result = (self.list.chunks)
                    .get(*chunk_index)
                    .and_then(|chunk| chunk.data.as_ref())
                    .and_then(|data| data.get(*item_index));

                *item_index += 1;
                if *item_index >= self.list.options.chunk_size {
                    *chunk_index += 1;
                    *item_index = 0;
                }
                result
            }
        };
        self.remaining -= 1;
        Some(result)
    }
}

impl<'a, T: 'a, P: PaginatedDataProvider> IntoIterator for &'a PaginatedList<T, P> {
    type Item = Option<&'a T>;
    type IntoIter = PaginatedListIterator<'a, T, P>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

fn pop_front_while<T>(vec: &mut VecDeque<T>, predicate: impl Fn(&T) -> bool) {
    while let Some(item) = vec.front() {
        if predicate(item) {
            vec.pop_front();
        } else {
            break;
        }
    }
}

fn pop_back_while<T>(vec: &mut VecDeque<T>, predicate: impl Fn(&T) -> bool) {
    while let Some(item) = vec.back() {
        if predicate(item) {
            vec.pop_back();
        } else {
            break;
        }
    }
}

#[derive(Debug)]
pub(crate) struct InteractiveList<T, P> {
    pub(crate) items: PaginatedList<T, P>,
    pub(crate) selected_index: usize,
    scroll_margin: usize,
}

impl<T, P: PaginatedDataProvider> InteractiveList<T, P> {
    pub(crate) fn new(items: PaginatedList<T, P>) -> Self {
        InteractiveList {
            items,
            selected_index: 0,
            scroll_margin: 0,
        }
    }

    pub(crate) fn with_margins(mut self, margins: usize) -> Self {
        self.scroll_margin = margins;
        self
    }

    pub(crate) fn move_cursor(&mut self, offset: i64) {
        if offset > 0 {
            self.selected_index =
                (self.selected_index + offset as usize).min(self.items.size.saturating_sub(1))
        } else {
            self.selected_index = self.selected_index.saturating_sub((-offset) as usize)
        }

        if self.selected_index < self.items.offset + self.scroll_margin {
            self.items
                .set_offset(self.selected_index.saturating_sub(self.scroll_margin));
        } else if self.selected_index
            > (self.items.offset + self.items.window_size).saturating_sub(self.scroll_margin + 1)
        {
            self.items.set_offset(
                (self.selected_index + self.scroll_margin + 1)
                    .saturating_sub(self.items.window_size),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{InteractiveList, PaginatedDataProvider, PaginatedList, PaginatedListOptions};
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;

    #[derive(Debug)]
    struct MockProvider {
        data: Rc<RefCell<VecDeque<usize>>>,
    }

    impl PaginatedDataProvider for MockProvider {
        fn request_page(&mut self, index: usize, _size: usize) {
            self.data.borrow_mut().push_back(index)
        }
    }

    #[test]
    fn initializing() {
        let requested = Rc::new(RefCell::new(VecDeque::new()));
        let provider = MockProvider {
            data: requested.clone(),
        };
        let options = PaginatedListOptions {
            chunk_size: 4,
            loaded_margin_size: 1,
        };
        let mut list = PaginatedList::new_with_options(6, 20, provider, options);
        assert_eq!(requested.borrow_mut().pop_front(), Some(0));
        assert_eq!(requested.borrow_mut().pop_front(), Some(1));
        assert_eq!(requested.borrow_mut().pop_front(), None);

        list.data_available(1, vec![4, 5, 6, 7]);
        assert_eq!(
            list.iter().collect::<Vec<Option<&i8>>>(),
            vec![None, None, None, None, Some(&4), Some(&5)]
        );

        list.data_available(0, vec![0, 1, 2, 3]);
        assert_eq!(
            list.iter().collect::<Vec<Option<&i8>>>(),
            vec![Some(&0), Some(&1), Some(&2), Some(&3), Some(&4), Some(&5)]
        );

        list.scroll(1);
        assert_eq!(
            list.iter().collect::<Vec<Option<&i8>>>(),
            vec![Some(&1), Some(&2), Some(&3), Some(&4), Some(&5), Some(&6)],
        );
        assert_eq!(requested.borrow_mut().pop_front(), None);

        list.scroll(1);
        assert_eq!(
            list.iter().collect::<Vec<Option<&i8>>>(),
            vec![Some(&2), Some(&3), Some(&4), Some(&5), Some(&6), Some(&7)],
        );
        assert_eq!(requested.borrow_mut().pop_front(), Some(2));

        list.scroll(1);
        assert_eq!(
            list.iter().collect::<Vec<Option<&i8>>>(),
            vec![Some(&3), Some(&4), Some(&5), Some(&6), Some(&7), None],
        );
        assert_eq!(requested.borrow_mut().pop_front(), None);
    }

    #[derive(Debug)]
    struct NoopProvider;

    impl PaginatedDataProvider for NoopProvider {
        fn request_page(&mut self, _index: usize, _size: usize) {}
    }

    #[test]
    fn creating_and_dropping_pages() {
        let options = PaginatedListOptions {
            chunk_size: 3,
            loaded_margin_size: 1,
        };
        let mut list = PaginatedList::<(), _>::new_with_options(4, 1000, NoopProvider, options);
        let expected_indices = vec![
            vec![0, 1],
            vec![0, 1],
            vec![0, 1, 2],
            vec![0, 1, 2],
            vec![1, 2],
            vec![1, 2, 3],
            vec![1, 2, 3],
            vec![2, 3],
            vec![2, 3, 4],
        ];

        for expected in &expected_indices {
            assert_eq!(&list.get_page_indices(), expected);
            list.scroll(1);
        }

        for expected in expected_indices.iter().rev() {
            list.scroll(-1);
            assert_eq!(&list.get_page_indices(), expected);
        }
    }

    #[test]
    fn does_not_scroll_past_boundary() {
        let mut list = PaginatedList::<u8, _>::new(3, 4, NoopProvider);
        list.data_available(0, vec![1, 2, 3, 4]);
        assert_eq!(
            list.iter().collect::<Vec<Option<&u8>>>(),
            vec![Some(&1), Some(&2), Some(&3)],
        );

        list.scroll(-1);
        assert_eq!(
            list.iter().collect::<Vec<Option<&u8>>>(),
            vec![Some(&1), Some(&2), Some(&3)],
        );

        list.scroll(1);
        assert_eq!(
            list.iter().collect::<Vec<Option<&u8>>>(),
            vec![Some(&2), Some(&3), Some(&4)],
        );

        list.scroll(1);
        assert_eq!(
            list.iter().collect::<Vec<Option<&u8>>>(),
            vec![Some(&2), Some(&3), Some(&4)],
        );
    }

    #[test]
    fn items_fewer_then_window_size() {
        let mut list = PaginatedList::<u8, _>::new(4, 3, NoopProvider);
        list.data_available(0, vec![1, 2, 3]);

        let expected = vec![Some(&1), Some(&2), Some(&3)];
        assert_eq!(list.iter().collect::<Vec<Option<&u8>>>(), expected);
        list.scroll(-1);
        assert_eq!(list.iter().collect::<Vec<Option<&u8>>>(), expected);
        list.scroll(1);
        assert_eq!(list.iter().collect::<Vec<Option<&u8>>>(), expected);
    }

    mod interactive_list {
        use super::*;

        fn assert_list(list: &InteractiveList<u8, NoopProvider>, index: usize, items: &[u8]) {
            assert_eq!(list.selected_index, index);
            assert_eq!(
                &list
                    .items
                    .iter()
                    .map(Option::unwrap)
                    .cloned()
                    .collect::<Vec<u8>>(),
                items
            );
        }

        #[test]
        fn moving_selection() {
            let mut list = PaginatedList::<u8, _>::new(4, 6, NoopProvider);
            list.data_available(0, vec![0, 1, 2, 3, 4, 5]);
            let mut list = InteractiveList::new(list).with_margins(1);

            assert_list(&list, 0, &[0, 1, 2, 3]);
            list.move_cursor(1);
            assert_list(&list, 1, &[0, 1, 2, 3]);
            list.move_cursor(1);
            assert_list(&list, 2, &[0, 1, 2, 3]);
            list.move_cursor(1);
            assert_list(&list, 3, &[1, 2, 3, 4]);
            list.move_cursor(1);
            assert_list(&list, 4, &[2, 3, 4, 5]);
            list.move_cursor(1);
            assert_list(&list, 5, &[2, 3, 4, 5]);
            list.move_cursor(1);
            assert_list(&list, 5, &[2, 3, 4, 5]);

            list.move_cursor(-1);
            assert_list(&list, 4, &[2, 3, 4, 5]);
            list.move_cursor(-1);
            assert_list(&list, 3, &[2, 3, 4, 5]);
            list.move_cursor(-1);
            assert_list(&list, 2, &[1, 2, 3, 4]);
            list.move_cursor(-1);
            assert_list(&list, 1, &[0, 1, 2, 3]);
            list.move_cursor(-1);
            assert_list(&list, 0, &[0, 1, 2, 3]);
            list.move_cursor(-1);
            assert_list(&list, 0, &[0, 1, 2, 3]);
        }

        #[test]
        fn all_on_screen() {
            let mut list = PaginatedList::<u8, _>::new(4, 4, NoopProvider);
            list.data_available(0, vec![0, 1, 2, 3]);
            let mut list = InteractiveList::new(list).with_margins(1);

            list.move_cursor(-1);
            assert_list(&list, 0, &[0, 1, 2, 3]);
            list.move_cursor(1);
            assert_list(&list, 1, &[0, 1, 2, 3]);
            list.move_cursor(1);
            assert_list(&list, 2, &[0, 1, 2, 3]);
            list.move_cursor(1);
            assert_list(&list, 3, &[0, 1, 2, 3]);
            list.move_cursor(1);
            assert_list(&list, 3, &[0, 1, 2, 3]);
        }
    }
}
