use std::collections::VecDeque;
use std::ops::Range;

#[derive(Debug, Clone)]
pub(crate) struct DataViewOptions {
    page_size: usize,
    load_margins: usize,
    scroll_margins: usize,
}

impl Default for DataViewOptions {
    fn default() -> Self {
        DataViewOptions {
            page_size: 128,
            load_margins: 32,
            scroll_margins: 3,
        }
    }
}

pub(crate) trait DataView {
    type Item;
    type Request;
    type Message;

    fn init(request_data: impl Fn(Self::Request), options: DataViewOptions) -> Self;
    fn item_at(&self, index: usize) -> Option<&Self::Item>;
    fn size(&self) -> Option<usize>;
    fn update(&mut self, range: Range<usize>, request_data: impl Fn(Self::Request));
    fn handle(&mut self, msg: Self::Message) -> bool;
}

#[derive(Debug)]
pub(crate) struct ListDataRequest;

#[derive(Debug)]
pub(crate) struct ListData<T> {
    items: Option<Vec<T>>,
}

impl<T> DataView for ListData<T> {
    type Item = T;
    type Request = ListDataRequest;
    type Message = Vec<T>;

    fn init(request_data: impl Fn(Self::Request), _options: DataViewOptions) -> Self {
        request_data(ListDataRequest);
        Self { items: None }
    }

    fn size(&self) -> Option<usize> {
        self.items.as_ref().map(Vec::len)
    }

    fn update(&mut self, _range: Range<usize>, _request_data: impl Fn(Self::Request)) {}

    fn handle(&mut self, msg: Self::Message) -> bool {
        if self.items.is_none() {
            self.items = Some(msg);
            true
        } else {
            false
        }
    }

    fn item_at(&self, index: usize) -> Option<&Self::Item> {
        self.items.as_ref().and_then(|items| items.get(index))
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum PaginatedDataRequest {
    Size,
    Page { index: usize, range: Range<usize> },
}

#[derive(Debug)]
pub(crate) enum PaginatedDataMessage<T> {
    Size(usize),
    Page { index: usize, values: Vec<T> },
}

impl<T> PaginatedDataMessage<T> {
    pub(crate) fn size(size: usize) -> Self {
        PaginatedDataMessage::Size(size)
    }

    pub(crate) fn page(index: usize, values: Vec<T>) -> Self {
        PaginatedDataMessage::Page { index, values }
    }
}

#[derive(Debug)]
pub(crate) struct PaginatedData<T> {
    page_size: usize,
    load_margins: usize,
    size: Option<usize>,
    first_page_index: usize,
    pages: VecDeque<Option<Vec<T>>>,
}

impl<T> PaginatedData<T> {
    fn page_index(&self, index: usize) -> usize {
        index / self.page_size
    }

    fn page_item_index(&self, index: usize) -> usize {
        index % self.page_size
    }

    fn request_page(&self, index: usize, request_data: &impl Fn(PaginatedDataRequest)) {
        request_data(PaginatedDataRequest::Page {
            index,
            range: (index * self.page_size)..((index + 1) * self.page_size),
        });
    }

    #[cfg(test)]
    fn pages_range(&self) -> (usize, usize) {
        (self.first_page_index, self.pages.len())
    }
}

impl<T> DataView for PaginatedData<T> {
    type Item = T;
    type Request = PaginatedDataRequest;
    type Message = PaginatedDataMessage<T>;

    fn init(request_data: impl Fn(Self::Request), options: DataViewOptions) -> Self {
        request_data(PaginatedDataRequest::Size);
        PaginatedData {
            page_size: options.page_size,
            load_margins: options.load_margins,
            size: None,
            first_page_index: 0,
            pages: VecDeque::new(),
        }
    }

    fn size(&self) -> Option<usize> {
        self.size
    }

    fn update(&mut self, range: Range<usize>, request_data: impl Fn(Self::Request)) {
        let size = match self.size {
            Some(size) => size,
            None => return,
        };
        let first_required_page = self.page_index(range.start.saturating_sub(self.load_margins));
        let last_required_page =
            self.page_index(((range.end + self.load_margins).saturating_sub(1)).min(size));
        let indices_count = last_required_page - first_required_page + 1;

        if !self.pages.is_empty() {
            while self.first_page_index < first_required_page {
                self.pages.pop_front();
                self.first_page_index += 1;
            }
            while self.first_page_index > first_required_page {
                self.pages.push_front(None);
                self.first_page_index -= 1;
                self.request_page(self.first_page_index, &request_data);
            }

            if self.pages.len() > indices_count {
                self.pages.drain(indices_count..);
            }
        } else {
            self.first_page_index = first_required_page;
        }
        while self.pages.len() < indices_count {
            self.request_page(self.first_page_index + self.pages.len(), &request_data);
            self.pages.push_back(None);
        }
    }

    fn handle(&mut self, msg: Self::Message) -> bool {
        match msg {
            PaginatedDataMessage::Size(size) => {
                self.size = Some(size);
                true
            }
            PaginatedDataMessage::Page { index, values } => {
                if index < self.first_page_index {
                    return false;
                }
                if let Some(page) = self.pages.get_mut(index - self.first_page_index) {
                    *page = Some(values);
                    true
                } else {
                    false
                }
            }
        }
    }

    fn item_at(&self, index: usize) -> Option<&T> {
        let page_index = self.page_index(index);
        if page_index < self.first_page_index {
            return None;
        }
        self.pages
            .get(page_index - self.first_page_index)
            .and_then(|page| page.as_ref())
            .and_then(|page| page.get(self.page_item_index(index)))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(transparent)]
pub(crate) struct Version(usize);

impl Version {
    fn advanced(&self) -> Version {
        Version(self.0.wrapping_add(1))
    }
}

impl Default for Version {
    fn default() -> Self {
        Version(0)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Versioned<T>(Version, T);

impl<T> Versioned<T> {
    pub(crate) fn new(value: T) -> Self {
        Versioned(Version::default(), value)
    }

    pub(crate) fn with_version(mut self, version: Version) -> Self {
        self.0 = version;
        self
    }

    pub(crate) fn update<R>(&self, new_value: R) -> Versioned<R> {
        Versioned(self.0.advanced(), new_value)
    }

    pub(crate) fn with_data<R>(&self, new_value: R) -> Versioned<R> {
        Versioned(self.0, new_value)
    }

    pub(crate) fn same_version<R>(&self, other: &Versioned<R>) -> bool {
        self.0 == other.0
    }

    pub(crate) fn as_ref(&self) -> Versioned<&T> {
        Versioned(self.0, &self.1)
    }

    pub(crate) fn map<R>(self, f: impl FnOnce(T) -> R) -> Versioned<R> {
        Versioned(self.0, f(self.1))
    }

    #[cfg(test)]
    pub(crate) fn as_inner(&self) -> &T {
        &self.1
    }

    pub(crate) fn version(&self) -> Version {
        self.0
    }

    pub(crate) fn into_inner(self) -> T {
        self.1
    }

    pub(crate) fn deconstruct(self) -> (Version, T) {
        (self.0, self.1)
    }
}

pub(crate) trait DataProvider {
    type Request;

    fn request(&self, request: Versioned<Self::Request>);
}

#[derive(Debug)]
pub(crate) struct InteractiveList<T: DataView, P: DataProvider<Request = T::Request>> {
    provider: Versioned<Option<P>>,
    data: T,
    options: DataViewOptions,
    selection: usize,
    offset: usize,
    window_size: usize,
}

impl<T: DataView, P: DataProvider<Request = T::Request>> InteractiveList<T, P> {
    pub(crate) fn new(window_size: usize) -> Self {
        Self::new_with_options(window_size, DataViewOptions::default())
    }

    pub(crate) fn new_with_options(window_size: usize, options: DataViewOptions) -> Self {
        InteractiveList {
            provider: Versioned::new(None),
            data: T::init(|_| (), options.clone()),
            options,
            selection: 0,
            offset: 0,
            window_size,
        }
    }

    pub(crate) fn set_provider(&mut self, provider: P) {
        self.provider = self.provider.update(Some(provider));
        self.offset = 0;
        self.selection = 0;
        self.data = T::init(
            |request| request_data(&self.provider, request),
            self.options.clone(),
        );
    }

    pub(crate) fn update_provider(&mut self, update: impl FnOnce(&mut P)) -> bool {
        let provider = self.provider.1.take();
        if let Some(mut provider) = provider {
            update(&mut provider);
            self.set_provider(provider);
            true
        } else {
            false
        }
    }

    pub(crate) fn selection(&self) -> Option<&T::Item> {
        self.data.item_at(self.selection)
    }

    fn update(&mut self) {
        let provider = &self.provider;
        self.data
            .update(self.offset..(self.offset + self.window_size), |request| {
                request_data(provider, request)
            });
    }

    pub(crate) fn set_window_size(&mut self, window_size: usize) {
        self.window_size = window_size;
        self.move_cursor(0);
    }

    pub(crate) fn handle_data(&mut self, msg: Versioned<T::Message>) -> bool {
        let previous_size = self.data.size();
        if !self.provider.same_version(&msg) || !self.data.handle(msg.into_inner()) {
            return false;
        };
        if previous_size.is_none() && self.data.size().is_some() {
            self.update();
        }
        true
    }

    fn set_cursor(&mut self, position: usize) {
        let size = match self.data.size() {
            Some(size) => size,
            None => return,
        };
        self.selection = position;

        let new_offset = if self.selection < self.offset + self.options.scroll_margins {
            Some(self.selection.saturating_sub(self.options.scroll_margins))
        } else if self.selection
            > (self.offset + self.window_size).saturating_sub(self.options.scroll_margins + 1)
        {
            Some(
                (self.selection + self.options.scroll_margins + 1).saturating_sub(self.window_size),
            )
        } else {
            None
        };

        if let Some(offset) = new_offset {
            let provider = &self.provider;
            self.offset = offset.min(size.saturating_sub(self.window_size));
            self.data
                .update(offset..(offset + self.window_size), |request| {
                    request_data(provider, request)
                });
        }
    }

    pub(crate) fn move_cursor(&mut self, offset: isize) {
        let size = match self.data.size() {
            Some(size) => size,
            None => return,
        };
        if offset < 0 {
            self.set_cursor(self.selection.saturating_sub(offset.abs() as usize));
        } else {
            self.set_cursor(
                (self.selection)
                    .saturating_add(offset as usize)
                    .min(size.saturating_sub(1)),
            );
        }
    }

    pub(crate) fn move_cursor_first(&mut self) {
        self.set_cursor(0)
    }

    pub(crate) fn move_cursor_last(&mut self) {
        if let Some(size) = self.data.size() {
            self.set_cursor(size.saturating_sub(1));
        }
    }

    pub(crate) fn iter(&self) -> Option<impl Iterator<Item = (Option<&T::Item>, bool)>> {
        let window_size = self.window_size;
        let offset = self.offset;
        let selection = self.selection;
        self.data.size().map(move |size| {
            (offset..(offset + window_size).min(size))
                .map(move |index| (self.data.item_at(index), index == selection))
        })
    }

    pub(crate) fn handle_command(&mut self, command: CursorCommand) {
        match command {
            CursorCommand::Next => self.move_cursor(1),
            CursorCommand::Previous => self.move_cursor(-1),
            CursorCommand::First => self.move_cursor_first(),
            CursorCommand::Last => self.move_cursor_last(),
        }
    }
}

#[derive(Debug, serde::Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CursorCommand {
    Next,
    Previous,
    First,
    Last,
}

fn request_data<P: DataProvider>(provider: &Versioned<Option<P>>, message: P::Request) {
    let message = provider.with_data(message);
    provider.as_ref().map(|provider| {
        if let Some(provider) = provider {
            provider.request(message)
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{
        DataProvider, DataView, DataViewOptions, InteractiveList, ListData, PaginatedData,
        PaginatedDataMessage, PaginatedDataRequest, Versioned,
    };
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;

    const TEST_OPTIONS: DataViewOptions = DataViewOptions {
        page_size: 4,
        load_margins: 1,
        scroll_margins: 1,
    };

    #[derive(Debug)]
    struct MockDataProvider<T> {
        requests: Rc<RefCell<VecDeque<Versioned<T>>>>,
    }

    impl<T> MockDataProvider<T> {
        fn new() -> (Self, Rc<RefCell<VecDeque<Versioned<T>>>>) {
            let requests = Rc::new(RefCell::new(VecDeque::new()));
            let provider = MockDataProvider {
                requests: requests.clone(),
            };
            (provider, requests)
        }
    }

    impl<T> DataProvider for MockDataProvider<T> {
        type Request = T;

        fn request(&self, request: Versioned<Self::Request>) {
            self.requests.borrow_mut().push_back(request);
        }
    }

    fn assert_list<P: DataProvider>(
        list: &InteractiveList<impl DataView<Item = u8, Request = P::Request>, P>,
        expected: &[(Option<u8>, bool)],
    ) {
        assert_eq!(
            list.iter()
                .unwrap()
                .map(|(a, b)| (a.cloned(), b))
                .collect::<Vec<(Option<u8>, bool)>>()
                .as_slice(),
            expected,
        );
    }

    macro_rules! item {
        ($value:expr) => {
            (Some($value), false)
        };
        ($value:expr, selected) => {
            (Some($value), true)
        };
    }

    macro_rules! no_item {
        () => {
            (None, false)
        };
        (selected) => {
            (None, true)
        };
    }

    #[test]
    fn scrolling_list_data() {
        let mut scroll_list =
            InteractiveList::<ListData<u8>, MockDataProvider<_>>::new_with_options(4, TEST_OPTIONS);
        assert!(scroll_list.iter().is_none());

        let (provider, requests) = MockDataProvider::new();
        scroll_list.set_provider(provider);
        assert!(scroll_list.iter().is_none());
        let request = requests.borrow_mut().pop_front().unwrap();
        assert!(requests.borrow().is_empty());
        requests.borrow_mut().clear();

        scroll_list.handle_data(request.with_data(vec![1, 2, 3, 4, 5, 6]));
        let expected_forward = vec![
            [item!(1, selected), item!(2), item!(3), item!(4)],
            [item!(1), item!(2, selected), item!(3), item!(4)],
            [item!(1), item!(2), item!(3, selected), item!(4)],
            [item!(2), item!(3), item!(4, selected), item!(5)],
            [item!(3), item!(4), item!(5, selected), item!(6)],
            [item!(3), item!(4), item!(5), item!(6, selected)],
            [item!(3), item!(4), item!(5), item!(6, selected)],
        ];
        for expected in expected_forward {
            assert_list(&scroll_list, &expected);
            scroll_list.move_cursor(1);
        }
        assert_eq!(scroll_list.selection(), Some(&6));

        let expected_backward = vec![
            [item!(3), item!(4), item!(5), item!(6, selected)],
            [item!(3), item!(4), item!(5, selected), item!(6)],
            [item!(3), item!(4, selected), item!(5), item!(6)],
            [item!(2), item!(3, selected), item!(4), item!(5)],
            [item!(1), item!(2, selected), item!(3), item!(4)],
            [item!(1, selected), item!(2), item!(3), item!(4)],
            [item!(1, selected), item!(2), item!(3), item!(4)],
        ];
        for expected in expected_backward {
            assert_list(&scroll_list, &expected);
            scroll_list.move_cursor(-1);
        }
        assert_eq!(scroll_list.selection(), Some(&1));
    }

    #[test]
    fn scrolling_fits_on_screen() {
        let mut scroll_list =
            InteractiveList::<ListData<u8>, MockDataProvider<_>>::new_with_options(4, TEST_OPTIONS);
        assert!(scroll_list.iter().is_none());

        let (provider, requests) = MockDataProvider::new();
        scroll_list.set_provider(provider);
        assert!(scroll_list.iter().is_none());
        let request = requests.borrow_mut().pop_front().unwrap();
        assert!(requests.borrow().is_empty());
        requests.borrow_mut().clear();

        scroll_list.handle_data(request.with_data(vec![1, 2, 3]));
        let expected_both_ways = vec![
            [item!(1, selected), item!(2), item!(3)],
            [item!(1, selected), item!(2), item!(3)],
            [item!(1), item!(2, selected), item!(3)],
            [item!(1), item!(2), item!(3, selected)],
            [item!(1), item!(2), item!(3, selected)],
        ];
        for expected in expected_both_ways.iter().skip(1) {
            assert_list(&scroll_list, expected);
            scroll_list.move_cursor(1);
        }
        for expected in expected_both_ways.iter().rev().skip(1) {
            assert_list(&scroll_list, expected);
            scroll_list.move_cursor(-1);
        }
    }

    #[test]
    fn initializing_paginated_list() {
        let mut scroll_list =
            InteractiveList::<PaginatedData<u8>, MockDataProvider<_>>::new_with_options(
                6,
                TEST_OPTIONS,
            );
        assert!(scroll_list.iter().is_none());

        let (provider, requests) = MockDataProvider::new();
        scroll_list.set_provider(provider);
        assert!(scroll_list.iter().is_none());

        let request = requests.borrow_mut().pop_front().unwrap();
        assert_eq!(request.as_inner(), &PaginatedDataRequest::Size);
        scroll_list.handle_data(request.with_data(PaginatedDataMessage::Size(20)));

        assert_list(
            &scroll_list,
            &[
                no_item!(selected),
                no_item!(),
                no_item!(),
                no_item!(),
                no_item!(),
                no_item!(),
            ],
        );

        assert_eq!(
            requests.borrow_mut().pop_front().unwrap().into_inner(),
            PaginatedDataRequest::Page {
                index: 0,
                range: 0..4
            }
        );
        assert_eq!(
            requests.borrow_mut().pop_front().unwrap().into_inner(),
            PaginatedDataRequest::Page {
                index: 1,
                range: 4..8
            }
        );

        scroll_list.handle_data(request.with_data(PaginatedDataMessage::Page {
            index: 1,
            values: vec![4, 5, 6, 7],
        }));
        assert_list(
            &scroll_list,
            &[
                no_item!(selected),
                no_item!(),
                no_item!(),
                no_item!(),
                item!(4),
                item!(5),
            ],
        );

        scroll_list.handle_data(request.with_data(PaginatedDataMessage::Page {
            index: 0,
            values: vec![0, 1, 2, 3],
        }));
        assert_list(
            &scroll_list,
            &[
                item!(0, selected),
                item!(1),
                item!(2),
                item!(3),
                item!(4),
                item!(5),
            ],
        );

        scroll_list.move_cursor(6);
        assert_list(
            &scroll_list,
            &[
                item!(2),
                item!(3),
                item!(4),
                item!(5),
                item!(6, selected),
                item!(7),
            ],
        );
        assert_eq!(
            requests.borrow_mut().pop_front().unwrap().into_inner(),
            PaginatedDataRequest::Page {
                index: 2,
                range: 8..12
            }
        );

        scroll_list.move_cursor(1);
        assert!(requests.borrow().is_empty());
        assert_list(
            &scroll_list,
            &[
                item!(3),
                item!(4),
                item!(5),
                item!(6),
                item!(7, selected),
                no_item!(),
            ],
        );
    }

    #[test]
    fn creating_and_dropping_pages() {
        let mut options = TEST_OPTIONS.clone();
        options.page_size = 3;
        let mut scroll_list =
            InteractiveList::<PaginatedData<u8>, MockDataProvider<_>>::new_with_options(4, options);

        let (provider, requests) = MockDataProvider::new();
        scroll_list.set_provider(provider);
        assert!(scroll_list.iter().is_none());

        let request = requests.borrow_mut().pop_front().unwrap();
        assert_eq!(request.as_inner(), &PaginatedDataRequest::Size);
        scroll_list.handle_data(request.with_data(PaginatedDataMessage::Size(100)));

        let scrolling_data = vec![
            (0, 2, [item!(0, selected), item!(0), item!(0), item!(1)]),
            (0, 2, [item!(0), item!(0, selected), item!(0), item!(1)]),
            (0, 2, [item!(0), item!(0), item!(0, selected), item!(1)]),
            (0, 2, [item!(0), item!(0), item!(1, selected), item!(1)]),
            (0, 3, [item!(0), item!(1), item!(1, selected), item!(1)]),
            (0, 3, [item!(1), item!(1), item!(1, selected), item!(2)]),
            (1, 2, [item!(1), item!(1), item!(2, selected), item!(2)]),
            (1, 3, [item!(1), item!(2), item!(2, selected), item!(2)]),
            (1, 3, [item!(2), item!(2), item!(2, selected), item!(3)]),
            (2, 2, [item!(2), item!(2), item!(3, selected), item!(3)]),
        ];
        for (page_index, offset, expected) in scrolling_data {
            while let Some(request) = requests.borrow_mut().pop_front() {
                match request.as_inner() {
                    PaginatedDataRequest::Size => panic!(),
                    PaginatedDataRequest::Page { index, range } => {
                        scroll_list.handle_data(request.with_data(PaginatedDataMessage::Page {
                            index: *index,
                            values: vec![*index as u8; range.len()],
                        }));
                    }
                }
            }
            assert_list(&scroll_list, &expected);
            let (actual_index, actual_offset) = scroll_list.data.pages_range();
            assert_eq!(page_index, actual_index);
            assert_eq!(offset, actual_offset);

            scroll_list.move_cursor(1);
        }

        let scrolling_backwards_data = vec![
            (2, 3, [item!(2), item!(3), item!(3, selected), item!(3)]),
            (2, 3, [item!(2), item!(3, selected), item!(3), item!(3)]),
            (2, 2, [item!(2), item!(2, selected), item!(3), item!(3)]),
            (1, 3, [item!(2), item!(2, selected), item!(2), item!(3)]),
            (1, 3, [item!(1), item!(2, selected), item!(2), item!(2)]),
            (1, 2, [item!(1), item!(1, selected), item!(2), item!(2)]),
            (0, 3, [item!(1), item!(1, selected), item!(1), item!(2)]),
            (0, 3, [item!(0), item!(1, selected), item!(1), item!(1)]),
            (0, 2, [item!(0), item!(0, selected), item!(1), item!(1)]),
            (0, 2, [item!(0), item!(0, selected), item!(0), item!(1)]),
            (0, 2, [item!(0, selected), item!(0), item!(0), item!(1)]),
        ];
        for (page_index, offset, expected) in scrolling_backwards_data {
            while let Some(request) = requests.borrow_mut().pop_front() {
                match request.as_inner() {
                    PaginatedDataRequest::Size => panic!(),
                    PaginatedDataRequest::Page { index, range } => {
                        scroll_list.handle_data(request.with_data(PaginatedDataMessage::Page {
                            index: *index,
                            values: vec![*index as u8; range.len()],
                        }));
                    }
                }
            }
            assert_list(&scroll_list, &expected);
            let (actual_index, actual_offset) = scroll_list.data.pages_range();
            assert_eq!(page_index, actual_index);
            assert_eq!(offset, actual_offset);

            scroll_list.move_cursor(-1);
        }
    }
}
