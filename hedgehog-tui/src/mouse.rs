use tui::layout::Rect;

fn rect_contains(rect: &Rect, row: u16, column: u16) -> bool {
    (rect.left()..rect.right()).contains(&column) && (rect.top()..rect.bottom()).contains(&row)
}

#[derive(Default)]
pub(crate) struct WidgetPositions {
    episodes_list: Option<Rect>,
    feeds_list: Option<Rect>,
    search_list: Option<Rect>,
}

#[allow(clippy::enum_variant_names)]
pub(crate) enum MouseHitResult {
    FeedsRow(usize),
    EpisodesRow(usize),
    SearchRow(usize),
}

impl WidgetPositions {
    pub(crate) fn hit_test_at(&self, row: u16, column: u16) -> Option<MouseHitResult> {
        if let Some(feeds_list) = self.feeds_list {
            if rect_contains(&feeds_list, row, column) {
                return Some(MouseHitResult::FeedsRow((row - feeds_list.y) as usize));
            }
        }
        if let Some(episodes_list) = self.episodes_list {
            if rect_contains(&episodes_list, row, column) {
                return Some(MouseHitResult::EpisodesRow(
                    (row - episodes_list.y) as usize,
                ));
            }
        }
        if let Some(search_list) = self.search_list {
            if rect_contains(&search_list, row, column) {
                return Some(MouseHitResult::SearchRow(
                    (row - search_list.y) as usize / 2,
                ));
            }
        }

        None
    }

    pub(crate) fn set_episodes_list(&mut self, rect: Rect) {
        self.episodes_list = Some(rect);
    }

    pub(crate) fn set_feeds_list(&mut self, rect: Rect) {
        self.feeds_list = Some(rect);
    }

    pub(crate) fn set_search_list(&mut self, rect: Rect) {
        self.search_list = Some(rect);
    }
}
