use super::list::ListItemRenderingDelegate;
use crate::{status::StatusLogEntry, theming};
use tui::style::Style;

pub(crate) struct ErrorLogRowRenderer<'t> {
    theme: &'t theming::Theme,
}

impl<'t> ErrorLogRowRenderer<'t> {
    pub(crate) fn new(theme: &'t theming::Theme) -> Self {
        ErrorLogRowRenderer { theme }
    }
}

impl<'t, 'a> ListItemRenderingDelegate<'a> for ErrorLogRowRenderer<'t> {
    type Item = (&'a StatusLogEntry, bool);

    fn render_item(
        &self,
        area: tui::layout::Rect,
        item: Self::Item,
        buf: &mut tui::buffer::Buffer,
    ) {
        let (item, selected) = item;
        let item_selector = theming::ListItem {
            selected,
            focused: true,
            state: Some(theming::ListState::LogEntry),
            ..Default::default()
        };
        let style = self.theme.get(theming::List::Item(item_selector));
        buf.set_style(area, style);

        buf.set_string(area.x, area.y, item.status().to_string(), Style::default());
    }

    fn render_empty(&self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let item_selector = theming::ListItem {
            focused: true,
            state: Some(theming::ListState::LogEntry),
            ..Default::default()
        };
        let style = self.theme.get(theming::List::Item(item_selector));
        buf.set_style(area, style);
    }
}
