use super::{
    layout::{shrink_h, split_left, split_top},
    list::ListItemRenderingDelegate,
};
use crate::{status::StatusLogEntry, theming};
use tui::{
    style::Style,
    widgets::{Paragraph, Widget, Wrap},
};
use unicode_width::UnicodeWidthStr;

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

        let status = item.status();

        let label = status
            .variant_label()
            .unwrap_or_else(|| match status.severity() {
                crate::status::Severity::Error => "Error",
                crate::status::Severity::Warning => "Warning",
                crate::status::Severity::Information => "Information",
            });
        buf.set_stringn(
            area.x + 1,
            area.y,
            label,
            area.width.saturating_sub(2) as usize,
            Style::default(),
        );

        let time = item.timestamp().format("  %X").to_string();
        let time_width = time.width();
        buf.set_stringn(
            area.right().saturating_sub((time_width + 1) as u16),
            area.y,
            time,
            time_width,
            Style::default(),
        );

        if area.height == 1 {
            return;
        }

        let paragraph = Paragraph::new(status.to_string()).wrap(Wrap { trim: true });
        paragraph.render(split_left(shrink_h(split_top(area, 1).1, 1), 2).1, buf);
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
