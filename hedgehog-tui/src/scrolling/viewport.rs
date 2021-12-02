use std::ops::Range;

pub(crate) struct Viewport {
    window_size: usize,
    items_count: usize,
    selected_item: usize,
    offset: usize,
    scroll_margin: usize,
}

impl Viewport {
    pub(crate) fn new(window_size: usize, items_count: usize) -> Self {
        Viewport {
            window_size,
            items_count,
            selected_item: 0,
            offset: 0,
            scroll_margin: 0,
        }
    }

    pub(crate) fn with_scroll_margin(mut self, margin: usize) -> Self {
        self.scroll_margin = margin;
        self
    }

    pub(crate) fn set_window_size(&mut self, window_size: usize) {
        self.window_size = window_size;
        self.ensure_visible();
    }

    pub(crate) fn update(&mut self, selection: usize, items_count: usize) {
        self.selected_item = selection;
        self.items_count = items_count;
        self.ensure_visible();
    }

    pub(crate) fn selected_index(&self) -> usize {
        self.selected_item
    }

    pub(crate) fn range(&self) -> Range<usize> {
        (self.offset)..(self.offset + self.window_size).min(self.items_count)
    }

    fn effective_scroll_margin(&self) -> usize {
        self.scroll_margin
            .min(self.window_size.saturating_sub(1) / 2)
    }

    fn scroll_range(&self) -> Range<usize> {
        let margin = self.effective_scroll_margin();
        (self.offset + margin)
            ..((self.offset + self.window_size).saturating_sub(margin)).min(self.items_count)
    }

    fn ensure_visible(&mut self) {
        let mut range = self.scroll_range();
        let margin = self.effective_scroll_margin();
        if (range.len() + margin * 2) < self.window_size && range.start > 0 {
            self.offset = self.items_count.saturating_sub(self.window_size);
            range = self.scroll_range();
        }

        if self.selected_item < range.start {
            let difference = range.start - self.selected_item;
            self.offset = self.offset.saturating_sub(difference);
        } else if self.selected_item >= range.end {
            let difference = (self.selected_item - range.end) + 1;
            self.offset =
                (self.offset + difference).min(self.items_count.saturating_sub(self.window_size));
        }
    }

    pub(crate) fn offset_selection_by(&mut self, offset: isize) {
        if offset > 0 {
            self.selected_item = self
                .selected_item
                .saturating_add(offset as usize)
                .min(self.items_count.saturating_sub(1));
        } else {
            self.selected_item = self.selected_item.saturating_sub(-offset as usize);
        }

        self.ensure_visible();
    }

    pub(crate) fn select(&mut self, selected_item: usize) {
        self.selected_item = selected_item;
        self.ensure_visible();
    }

    pub(crate) fn items_count(&self) -> usize {
        self.items_count
    }
}

#[cfg(test)]
mod tests {
    use super::Viewport;

    macro_rules! assert_scrolling {
        ($viewport:ident; $($offset:expr => ($start:literal..$end:literal, $selection:literal)),* $(,)?) => {{
            let step = 0;
            $(
                $viewport.offset_selection_by($offset);
                assert_eq!($viewport.selected_index(), $selection, "step {}", step);
                assert_eq!($viewport.range(), $start..$end, "step {}", step);
                let step = step + 1;
            )*
            let _ = step;
        }}
    }

    #[test]
    fn all_items_visible() {
        let mut viewport = Viewport::new(10, 5);
        assert_scrolling! {
            viewport;
            0 => (0..5, 0),
            1 => (0..5, 1),
            1 => (0..5, 2),
            1 => (0..5, 3),
            1 => (0..5, 4),
            1 => (0..5, 4),
            -1 => (0..5, 3),
            -1 => (0..5, 2),
            -1 => (0..5, 1),
            -1 => (0..5, 0),
            -1 => (0..5, 0),
        };
    }

    #[test]
    fn scrolling() {
        let mut viewport = Viewport::new(4, 6);
        assert_scrolling! {
            viewport;
            0 => (0..4, 0),
            1 => (0..4, 1),
            1 => (0..4, 2),
            1 => (0..4, 3),
            1 => (1..5, 4),
            1 => (2..6, 5),
            1 => (2..6, 5),
            -1 => (2..6, 4),
            -1 => (2..6, 3),
            -1 => (2..6, 2),
            -1 => (1..5, 1),
            -1 => (0..4, 0),
            -1 => (0..4, 0),
        };
    }

    #[test]
    fn scrolling_with_margins() {
        let mut viewport = Viewport::new(4, 6).with_scroll_margin(1);
        assert_scrolling! {
            viewport;
            0 => (0..4, 0),
            1 => (0..4, 1),
            1 => (0..4, 2),
            1 => (1..5, 3),
            1 => (2..6, 4),
            1 => (2..6, 5),
            1 => (2..6, 5),
            -1 => (2..6, 4),
            -1 => (2..6, 3),
            -1 => (1..5, 2),
            -1 => (0..4, 1),
            -1 => (0..4, 0),
            -1 => (0..4, 0),
        };
    }

    #[test]
    fn scrolling_margins_height_1() {
        let mut viewport = Viewport::new(1, 4).with_scroll_margin(2);
        assert_scrolling! {
            viewport;
            0 => (0..1, 0),
            1 => (1..2, 1),
            1 => (2..3, 2),
            1 => (3..4, 3),
            1 => (3..4, 3),
            -1 => (2..3, 2),
            -1 => (1..2, 1),
            -1 => (0..1, 0),
            -1 => (0..1, 0),
        };
    }

    #[test]
    fn scrolling_margins_height_2() {
        let mut viewport = Viewport::new(2, 5).with_scroll_margin(2);
        assert_scrolling! {
            viewport;
            0 => (0..2, 0),
            1 => (0..2, 1),
            1 => (1..3, 2),
            1 => (2..4, 3),
            1 => (3..5, 4),
            1 => (3..5, 4),
            -1 => (3..5, 3),
            -1 => (2..4, 2),
            -1 => (1..3, 1),
            -1 => (0..2, 0),
            -1 => (0..2, 0),
        };
    }

    #[test]
    fn scrolling_margins_height_3() {
        let mut viewport = Viewport::new(3, 6).with_scroll_margin(2);
        assert_scrolling! {
            viewport;
            0 => (0..3, 0),
            1 => (0..3, 1),
            1 => (1..4, 2),
            1 => (2..5, 3),
            1 => (3..6, 4),
            1 => (3..6, 5),
            1 => (3..6, 5),
            -1 => (3..6, 4),
            -1 => (2..5, 3),
            -1 => (1..4, 2),
            -1 => (0..3, 1),
            -1 => (0..3, 0),
        };
    }

    #[test]
    fn size_change() {
        let mut viewport = Viewport::new(4, 10);
        assert_eq!(viewport.selected_index(), 0);
        assert_eq!(viewport.range(), 0..4);

        viewport.select(8);
        assert_eq!(viewport.selected_index(), 8);
        assert_eq!(viewport.range(), 5..9);

        viewport.set_window_size(3);
        assert_eq!(viewport.selected_index(), 8);
        assert_eq!(viewport.range(), 6..9);

        let size_increase_cases = [
            (4, 6..10),
            (5, 5..10),
            (6, 4..10),
            (7, 3..10),
            (8, 2..10),
            (9, 1..10),
            (10, 0..10),
            (11, 0..10),
        ];
        for (size, range) in size_increase_cases {
            viewport.set_window_size(size);
            assert_eq!(viewport.selected_index(), 8);
            assert_eq!(viewport.range(), range);
        }
    }

    #[test]
    fn empty_viewport() {
        let mut viewport = Viewport::new(10, 0);
        assert_eq!(viewport.selected_index(), 0);
        assert_eq!(viewport.items_count(), 0);
        assert_scrolling!(viewport; 0 => (0..0, 0), 1 => (0..0, 0), -1 => (0..0, 0));
    }
}
