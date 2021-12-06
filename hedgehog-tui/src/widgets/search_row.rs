use super::list::ListItemRenderingDelegate;
use crate::theming::{self, Theme};
use hedgehog_library::search::SearchResult;
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::widgets::{Paragraph, Widget};

pub(crate) struct SearchResultRowRenderer<'t> {
    theme: &'t Theme,
}

impl<'t> SearchResultRowRenderer<'t> {
    pub(crate) fn new(theme: &'t Theme) -> Self {
        SearchResultRowRenderer { theme }
    }
}

impl<'t, 'a> ListItemRenderingDelegate<'a> for SearchResultRowRenderer<'t> {
    type Item = (&'a SearchResult, bool);

    fn render_item(&self, area: Rect, item: Self::Item, buf: &mut tui::buffer::Buffer) {
        let (item, selected) = item;
        let item_selector = theming::ListItem {
            selected,
            focused: true,
            ..Default::default()
        };
        let style = self.theme.get(theming::List::Item(item_selector));
        buf.set_style(area, style);

        let paragraph = Paragraph::new(item.title.as_str());
        paragraph.render(
            Rect::new(
                area.x + 1,
                area.y,
                area.width.saturating_sub(2),
                area.height,
            ),
            buf,
        );
    }

    fn render_empty(&self, area: Rect, buf: &mut Buffer) {
        let item_selector = theming::ListItem {
            focused: true,
            ..Default::default()
        };
        let style = self.theme.get(theming::List::Item(item_selector));
        buf.set_style(area, style);
    }
}
