pub(crate) mod interactive;
pub(crate) mod linear;
pub(crate) mod paginated;

use cmd_parser::CmdParsable;
use hedgehog_library::model::Identifiable;
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

pub(crate) trait EditableDataView {
    type Id;
    type Item: Identifiable<Id = Self::Id>;

    fn remove(&mut self, id: Self::Id) -> Option<usize>;
    fn add(&mut self, item: Self::Item);
}

pub(crate) trait UpdatableDataView {
    type Id;
    type Item: Identifiable<Id = Self::Id>;

    fn update(&mut self, id: Self::Id, callback: impl FnOnce(&mut Self::Item));
    fn update_all(&mut self, callback: impl Fn(&mut Self::Item));
    fn update_at(&mut self, index: usize, callback: impl FnOnce(&mut Self::Item));
}

fn index_with_id<'a, T: Identifiable + 'a>(
    items: impl Iterator<Item = &'a T>,
    id: T::Id,
) -> Option<usize> {
    items
        .enumerate()
        .filter(|(_, item)| item.id() == id)
        .map(|(index, _)| index)
        .next()
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

#[derive(Debug, Clone, Copy, PartialEq, CmdParsable)]
pub(crate) enum CursorCommand {
    Next,
    Previous,
    PageUp,
    PageDown,
    First,
    Last,
}

fn request_data<P: DataProvider>(provider: &Versioned<Option<P>>, message: P::Request) {
    let message = provider.with_data(message);
    provider.as_ref().map(|provider| {
        if let Some(provider) = provider {
            provider.request(message);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{
        interactive::InteractiveList, linear::ListData, paginated::PaginatedData,
        paginated::PaginatedDataMessage, paginated::PaginatedDataRequest, DataProvider, DataView,
        DataViewOptions, Versioned,
    };
    use hedgehog_library::model::Identifiable;
    use hedgehog_library::Page;
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

    fn assert_list<P: DataProvider, T: PartialEq + std::fmt::Debug + Clone>(
        list: &InteractiveList<impl DataView<Item = T, Request = P::Request>, P>,
        expected: &[(Option<T>, bool)],
    ) {
        assert_eq!(
            list.iter()
                .unwrap()
                .map(|(a, b)| (a.cloned(), b))
                .collect::<Vec<(Option<T>, bool)>>()
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
            PaginatedDataRequest::Page(Page::new(0, 4))
        );
        assert_eq!(
            requests.borrow_mut().pop_front().unwrap().into_inner(),
            PaginatedDataRequest::Page(Page::new(1, 4))
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
            PaginatedDataRequest::Page(Page::new(2, 4))
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
                    PaginatedDataRequest::Page(page) => {
                        scroll_list.handle_data(request.with_data(PaginatedDataMessage::Page {
                            index: page.index,
                            values: vec![page.index as u8; page.size],
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
                    PaginatedDataRequest::Page(page) => {
                        scroll_list.handle_data(request.with_data(PaginatedDataMessage::Page {
                            index: page.index,
                            values: vec![page.index as u8; page.size],
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

    #[derive(Debug, Clone, PartialEq)]
    struct IdItem(usize, &'static str);

    impl Identifiable for IdItem {
        type Id = usize;

        fn id(&self) -> Self::Id {
            self.0
        }
    }

    #[test]
    fn editing_data() {
        let mut scroll_list =
            InteractiveList::<ListData<IdItem>, MockDataProvider<_>>::new_with_options(
                10,
                TEST_OPTIONS,
            );

        let (provider, requests) = MockDataProvider::new();
        scroll_list.set_provider(provider);
        let request = requests.borrow_mut().pop_front().unwrap();
        scroll_list.handle_data(request.with_data(vec![
            IdItem(5, "five"),
            IdItem(3, "three"),
            IdItem(7, "seven"),
            IdItem(1, "one"),
        ]));

        scroll_list.add_item(IdItem(8, "eight"));
        assert_list(
            &scroll_list,
            &[
                item!(IdItem(5, "five"), selected),
                item!(IdItem(3, "three")),
                item!(IdItem(7, "seven")),
                item!(IdItem(1, "one")),
                item!(IdItem(8, "eight")),
            ],
        );

        scroll_list.replace_item(IdItem(3, "three v2"));
        assert_list(
            &scroll_list,
            &[
                item!(IdItem(5, "five"), selected),
                item!(IdItem(3, "three v2")),
                item!(IdItem(7, "seven")),
                item!(IdItem(1, "one")),
                item!(IdItem(8, "eight")),
            ],
        );

        scroll_list.remove_item(7);
        assert_list(
            &scroll_list,
            &[
                item!(IdItem(5, "five"), selected),
                item!(IdItem(3, "three v2")),
                item!(IdItem(1, "one")),
                item!(IdItem(8, "eight")),
            ],
        );
    }

    #[test]
    fn deleting_items() {
        let mut scroll_list =
            InteractiveList::<ListData<IdItem>, MockDataProvider<_>>::new_with_options(
                10,
                TEST_OPTIONS,
            );

        let (provider, requests) = MockDataProvider::new();
        scroll_list.set_provider(provider);
        let request = requests.borrow_mut().pop_front().unwrap();
        scroll_list.handle_data(request.with_data(vec![
            IdItem(1, "a"),
            IdItem(2, "b"),
            IdItem(3, "c"),
            IdItem(4, "d"),
            IdItem(5, "e"),
            IdItem(6, "f"),
            IdItem(7, "g"),
        ]));
        scroll_list.move_cursor(3);

        assert_list(
            &scroll_list,
            &[
                item!(IdItem(1, "a")),
                item!(IdItem(2, "b")),
                item!(IdItem(3, "c")),
                item!(IdItem(4, "d"), selected),
                item!(IdItem(5, "e")),
                item!(IdItem(6, "f")),
                item!(IdItem(7, "g")),
            ],
        );

        scroll_list.remove_item(6);
        assert_list(
            &scroll_list,
            &[
                item!(IdItem(1, "a")),
                item!(IdItem(2, "b")),
                item!(IdItem(3, "c")),
                item!(IdItem(4, "d"), selected),
                item!(IdItem(5, "e")),
                item!(IdItem(7, "g")),
            ],
        );

        scroll_list.remove_item(3);
        assert_list(
            &scroll_list,
            &[
                item!(IdItem(1, "a")),
                item!(IdItem(2, "b")),
                item!(IdItem(4, "d"), selected),
                item!(IdItem(5, "e")),
                item!(IdItem(7, "g")),
            ],
        );

        scroll_list.remove_item(4);
        assert_list(
            &scroll_list,
            &[
                item!(IdItem(1, "a")),
                item!(IdItem(2, "b")),
                item!(IdItem(5, "e"), selected),
                item!(IdItem(7, "g")),
            ],
        );

        scroll_list.remove_item(7);
        assert_list(
            &scroll_list,
            &[
                item!(IdItem(1, "a")),
                item!(IdItem(2, "b")),
                item!(IdItem(5, "e"), selected),
            ],
        );

        scroll_list.remove_item(5);
        assert_list(
            &scroll_list,
            &[item!(IdItem(1, "a")), item!(IdItem(2, "b"), selected)],
        );

        scroll_list.remove_item(2);
        assert_list(&scroll_list, &[item!(IdItem(1, "a"), selected)]);

        scroll_list.remove_item(1);
        assert_list(&scroll_list, &[]);

        scroll_list.add_item(IdItem(8, "h"));
        assert_list(&scroll_list, &[item!(IdItem(8, "h"), selected)]);
    }
}
