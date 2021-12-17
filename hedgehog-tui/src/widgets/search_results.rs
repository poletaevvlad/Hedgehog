use crate::{screen::SearchState, theming};
use tui::widgets::Widget;

use super::{empty::EmptyView, list::List, search_row::SearchResultRowRenderer};

pub(crate) struct SearchResults<'a> {
    search: &'a SearchState,
    theme: &'a theming::Theme,
}

impl<'a> SearchResults<'a> {
    pub(crate) fn new(search: &'a SearchState, theme: &'a theming::Theme) -> Self {
        SearchResults { search, theme }
    }
}

impl<'a> Widget for SearchResults<'a> {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        match &self.search {
            SearchState::Loaded(list) if list.data().is_empty() => EmptyView::new(self.theme)
                .title("Nothing is found")
                .subtitle("Please make sure that your query is correct")
                .focused(true)
                .render(area, buf),
            SearchState::Loaded(list) => {
                List::new(
                    SearchResultRowRenderer::new(self.theme),
                    list.visible_iter(),
                )
                .item_height(2)
                .render(area, buf);
            }
            SearchState::Loading => EmptyView::new(self.theme)
                .title("Searching...")
                .focused(true)
                .render(area, buf),
            SearchState::Error(err) => EmptyView::new(self.theme)
                .title("Search request failed")
                .subtitle(&err.to_string())
                .focused(true)
                .render(area, buf),
        }
    }
}
