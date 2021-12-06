use super::list::ListItemRenderingDelegate;
use crate::theming::{self, Theme};
use hedgehog_library::search::SearchResult;
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::text::{Span, Spans};
use tui::widgets::{Paragraph, Widget};
use unicode_width::UnicodeWidthStr;

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
            Rect::new(area.x + 1, area.y, area.width.saturating_sub(2), 1),
            buf,
        );

        if area.height > 1 {
            let metadata = Paragraph::new(vec![Spans::from(vec![
                Span::raw(&item.genre),
                Span::raw(", "),
                Span::raw("by "),
                Span::raw(&item.author),
            ])]);
            metadata.render(
                Rect::new(
                    area.x + 2,
                    area.y + 1,
                    area.width.saturating_sub(3),
                    area.height - 1,
                ),
                buf,
            );

            let episodes_count = format!("   {} ep. ", item.episodes_count);
            let episodes_count_width = episodes_count.width() as u16;
            buf.set_span(
                area.right().saturating_sub(episodes_count_width),
                area.y + 1,
                &Span::raw(&episodes_count),
                episodes_count_width,
            );
        }
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
