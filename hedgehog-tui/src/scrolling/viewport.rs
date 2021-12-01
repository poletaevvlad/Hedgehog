use std::ops::Range;

pub(crate) struct Viewport {
    visible_window_size: usize,
    effective_window_size: usize,
    items_count: usize,

    selected_item: usize,
    offset: usize,
}

impl Viewport {
    pub(crate) fn new(window_size: usize, effective_size: usize, items_count: usize) -> Self {
        Viewport {
            visible_window_size: window_size,
            effective_window_size: effective_size,
            items_count,
            selected_item: 0,
            offset: 0,
        }
    }

    pub(crate) fn set_window_size(&mut self, window_size: usize, effective_size: usize) {
        self.visible_window_size = window_size;
        self.effective_window_size = effective_size;
        self.ensure_visible();
    }

    pub(crate) fn selected_index(&self) -> usize {
        self.selected_item
    }

    pub(crate) fn effective_range(&self) -> Range<usize> {
        self.offset..(self.offset + self.effective_window_size).min(self.items_count)
    }

    pub(crate) fn visible_range(&self) -> Range<usize> {
        self.offset..(self.offset + self.visible_window_size).min(self.items_count)
    }

    fn ensure_visible(&mut self) {
        let mut range = self.visible_range();
        if range.len() < self.visible_window_size && range.start > 0 {
            self.offset = self.items_count.saturating_sub(self.visible_window_size);
            range = self.visible_range();
        }

        if self.selected_item < range.start {
            let difference = range.start - self.selected_item;
            self.offset = self.offset.saturating_sub(difference);
        } else if self.selected_item >= range.end {
            let difference = (self.selected_item - range.end) + 1;
            self.offset += difference;
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
                assert_eq!($viewport.effective_range(), $start..$end, "step {}", step);
                let step = step + 1;
            )*
            let _ = step;
        }}
    }

    #[test]
    fn all_items_visible() {
        let mut viewport = Viewport::new(10, 10, 5);
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
        let mut viewport = Viewport::new(4, 4, 6);
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
    fn scrolling_with_effective_height() {
        let mut viewport = Viewport::new(4, 5, 7);
        assert_scrolling! {
            viewport;
            0 => (0..5, 0),
            1 => (0..5, 1),
            1 => (0..5, 2),
            1 => (0..5, 3),
            1 => (1..6, 4),
            1 => (2..7, 5),
            1 => (3..7, 6),
            1 => (3..7, 6),
            -1 => (3..7, 5),
            -1 => (3..7, 4),
            -1 => (3..7, 3),
            -1 => (2..7, 2),
            -1 => (1..6, 1),
            -1 => (0..5, 0),
            -1 => (0..5, 0),
        };
    }

    #[test]
    fn size_change() {
        let mut viewport = Viewport::new(4, 4, 10);
        assert_eq!(viewport.selected_index(), 0);
        assert_eq!(viewport.effective_range(), 0..4);

        viewport.select(8);
        assert_eq!(viewport.selected_index(), 8);
        assert_eq!(viewport.effective_range(), 5..9);

        viewport.set_window_size(3, 3);
        assert_eq!(viewport.selected_index(), 8);
        assert_eq!(viewport.effective_range(), 6..9);

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
            viewport.set_window_size(size, size);
            assert_eq!(viewport.selected_index(), 8);
            assert_eq!(viewport.effective_range(), range);
        }
    }
}
