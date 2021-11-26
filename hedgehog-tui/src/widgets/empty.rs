use crate::theming::Theme;
use tui::layout::Alignment;
use tui::widgets::{Paragraph, Widget, Wrap};

use super::layout::split_top;

pub(crate) struct EmptyView<'t> {
    theme: &'t Theme,
    title: &'t str,
}

impl<'t> EmptyView<'t> {
    pub(crate) fn new(theme: &'t Theme) -> Self {
        EmptyView { theme, title: "" }
    }

    pub(crate) fn title(mut self, title: &'t str) -> Self {
        self.title = title;
        self
    }
}

impl<'t> Widget for EmptyView<'t> {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let (_, area) = split_top(area, 5.min(area.height / 5));

        Paragraph::new(self.title)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}
