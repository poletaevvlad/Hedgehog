pub(crate) mod pagination;
pub(crate) mod selection;
mod viewport;

use cmd_parser::CmdParsable;
use std::ops::Range;
use viewport::Viewport;

pub(crate) trait DataView {
    type Item;

    fn size(&self) -> usize;
    fn item_at(&self, index: usize) -> Option<&Self::Item>;
    fn find(&self, p: impl Fn(&Self::Item) -> bool) -> Option<usize>;
    fn prepare(&mut self, range: Range<usize>) {
        let _ = range;
    }
}

impl<T> DataView for Vec<T> {
    type Item = T;

    fn size(&self) -> usize {
        self.len()
    }

    fn item_at(&self, index: usize) -> Option<&T> {
        self.get(index)
    }

    fn find(&self, p: impl Fn(&Self::Item) -> bool) -> Option<usize> {
        self.iter()
            .enumerate()
            .find(|(_, item)| p(item))
            .map(|(index, _)| index)
    }
}

pub(crate) struct ScrollableList<D> {
    data: D,
    viewport: Viewport,
}

impl<D> ScrollableList<D> {
    pub(crate) fn data(&self) -> &D {
        &self.data
    }
}

impl<D: DataView> ScrollableList<D> {
    pub(crate) fn new(data: D, window_size: usize, margins: usize) -> Self {
        ScrollableList {
            viewport: Viewport::new(window_size, data.size()).with_scroll_margin(margins),
            data,
        }
    }

    pub(crate) fn set_window_size(&mut self, window_size: usize) {
        self.viewport.set_window_size(window_size);
        self.data.prepare(self.viewport.range());
    }

    pub(crate) fn viewport(&self) -> &Viewport {
        &self.viewport
    }

    pub(crate) fn visible_iter(&self) -> impl Iterator<Item = (&D::Item, bool)> {
        self.visible_iter_partial()
            .map(|(item, selected)| (item.unwrap(), selected))
    }

    pub(crate) fn visible_iter_partial(&self) -> impl Iterator<Item = (Option<&D::Item>, bool)> {
        let start = self.viewport.range().start;
        let size = self.viewport.items_count();
        let selection = self.viewport.selected_index();
        (start..size).map(move |index| (self.data.item_at(index), index == selection))
    }

    pub(crate) fn update_data<SelectionUpdate: selection::UpdateStrategy<D>, F: FnOnce(&mut D)>(
        &mut self,
        f: F,
    ) {
        let update_tmp = SelectionUpdate::before_update(&self.viewport, &self.data);
        f(&mut self.data);
        SelectionUpdate::update(&mut self.viewport, &self.data, update_tmp);
        self.data.prepare(self.viewport.range());
    }

    pub(crate) fn selection(&self) -> Option<&D::Item> {
        self.data.item_at(self.viewport.selected_index())
    }

    pub(crate) fn scroll(&mut self, action: ScrollAction) -> bool {
        let is_valid = match action {
            ScrollAction::MoveBy(offset) => {
                self.viewport.offset_selection_by(offset);
                self.viewport.items_count() > 0
            }
            ScrollAction::MoveToVisible(position) => {
                if let Some(offset) = self.viewport.range().next() {
                    if offset + position < self.viewport.items_count() {
                        self.viewport.select(offset + position);
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            ScrollAction::PageUp => {
                self.viewport
                    .offset_selection_by(self.viewport.window_size() as isize);
                self.viewport.items_count() > 0
            }
            ScrollAction::PageDown => {
                self.viewport
                    .offset_selection_by(-(self.viewport.window_size() as isize));
                self.viewport.items_count() > 0
            }
            ScrollAction::First => {
                self.viewport.select(0);
                self.viewport.items_count() > 0
            }
            ScrollAction::Last => {
                self.viewport
                    .select(self.viewport.items_count().saturating_sub(1));
                self.viewport.items_count() > 0
            }
        };
        self.data.prepare(self.viewport.range());
        is_valid
    }
}

#[derive(Debug, Clone, Copy, PartialEq, CmdParsable)]
pub(crate) enum ScrollAction {
    MoveBy(isize),
    MoveToVisible(usize),
    PageUp,
    PageDown,
    First,
    Last,
}

#[cfg(test)]
mod tests {
    use super::{selection, ScrollAction, ScrollableList};
    use hedgehog_library::model::Identifiable;

    #[test]
    fn empty() {
        let list = ScrollableList::new(Vec::<usize>::new(), 3, 0);
        assert!(list.visible_iter().next().is_none());
        assert_eq!(list.selection(), None);
    }

    #[test]
    fn iter() {
        let list = ScrollableList::new(vec![1, 2, 3, 4, 5], 3, 0);
        assert!(list.visible_iter().eq([
            (&1, true),
            (&2, false),
            (&3, false),
            (&4, false),
            (&5, false)
        ]
        .into_iter()));
        assert_eq!(list.selection(), Some(&1));
    }

    #[test]
    fn scroll() {
        let mut list = ScrollableList::new(vec![1, 2, 3, 4, 5], 3, 0);

        list.scroll(ScrollAction::Last);
        assert_eq!(list.selection(), Some(&5));
        assert!(list
            .visible_iter()
            .map(|x| x.0)
            .eq([&3, &4, &5].into_iter()));

        list.scroll(ScrollAction::MoveBy(-1));
        list.scroll(ScrollAction::MoveBy(-1));
        list.scroll(ScrollAction::MoveBy(-1));
        assert_eq!(list.selection(), Some(&2));
        assert!(list
            .visible_iter()
            .map(|x| x.0)
            .eq([&2, &3, &4, &5].into_iter()));

        list.scroll(ScrollAction::MoveBy(1));
        assert_eq!(list.selection(), Some(&3));
        assert!(list
            .visible_iter()
            .map(|x| x.0)
            .eq([&2, &3, &4, &5].into_iter()));

        list.scroll(ScrollAction::First);
        assert_eq!(list.selection(), Some(&1));
        assert!(list
            .visible_iter()
            .map(|x| x.0)
            .eq([&1, &2, &3, &4, &5].into_iter()));
    }

    #[derive(Debug, PartialEq, Eq)]
    struct Item<T>(usize, T);

    impl<T> Identifiable for Item<T> {
        type Id = usize;

        fn id(&self) -> Self::Id {
            self.0
        }
    }

    fn make_update_list() -> ScrollableList<Vec<Item<char>>> {
        let items_before = vec![Item(0, 'a'), Item(1, 'b'), Item(2, 'c'), Item(3, 'd')];
        let mut list = ScrollableList::new(items_before, 10, 0);
        list.scroll(ScrollAction::MoveBy(1));
        list.scroll(ScrollAction::MoveBy(1));
        assert_eq!(list.selection(), Some(&Item(2, 'c')));
        list
    }

    #[test]
    fn update_keep() {
        let mut list = make_update_list();
        let items_after = vec![Item(2, 'c'), Item(3, 'd'), Item(0, 'a'), Item(1, 'd')];
        list.update_data::<selection::Keep, _>(|data| *data = items_after);
        assert_eq!(list.selection(), Some(&Item(0, 'a')));
    }

    #[test]
    fn update_keep_size_reduced() {
        let mut list = make_update_list();
        let items_after = vec![Item(2, 'c'), Item(3, 'd')];
        list.update_data::<selection::Keep, _>(|data| *data = items_after);
        assert_eq!(list.selection(), Some(&Item(3, 'd')));
    }

    #[test]
    fn update_reset() {
        let mut list = make_update_list();
        let items_after = vec![Item(1, 'b'), Item(2, 'c'), Item(3, 'd'), Item(0, 'a')];
        list.update_data::<selection::Reset, _>(|data| *data = items_after);
        assert_eq!(list.selection(), Some(&Item(1, 'b')));
    }

    #[test]
    fn update_find() {
        let mut list = make_update_list();
        let items_after = vec![Item(1, 'b'), Item(2, 'c'), Item(3, 'd'), Item(0, 'a')];
        list.update_data::<selection::FindPrevious, _>(|data| *data = items_after);
        assert_eq!(list.selection(), Some(&Item(2, 'c')));
    }

    #[test]
    fn update_find_not_find() {
        let mut list = make_update_list();
        let items_after = vec![Item(1, 'b'), Item(4, 'e'), Item(3, 'd'), Item(0, 'a')];
        list.update_data::<selection::FindPrevious, _>(|data| *data = items_after);
        assert_eq!(list.selection(), Some(&Item(1, 'b')));
    }
}
