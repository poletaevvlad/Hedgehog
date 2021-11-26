use crate::theming::{self, Theme};
use tui::layout::Alignment;
use tui::text::Text;
use tui::widgets::{Paragraph, Widget, Wrap};

use super::layout::split_top;

pub(crate) struct EmptyView<'t> {
    theme: &'t Theme,
    title: &'t str,
    subtitle: &'t str,
}

impl<'t> EmptyView<'t> {
    pub(crate) fn new(theme: &'t Theme) -> Self {
        EmptyView {
            theme,
            title: "",
            subtitle: "",
        }
    }

    pub(crate) fn title(mut self, title: &'t str) -> Self {
        self.title = title;
        self
    }

    pub(crate) fn subtitle(mut self, subtitle: &'t str) -> Self {
        self.subtitle = subtitle;
        self
    }
}

impl<'t> Widget for EmptyView<'t> {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        buf.set_style(area, self.theme.get(theming::Empty::View));
        let (_, area) = split_top(area, 10.min(area.height / 5));

        let mut text = Text::styled(self.title, self.theme.get(theming::Empty::Title));
        if !self.subtitle.is_empty() {
            text.extend(Text::raw("\n"));
            text.extend(Text::styled(
                self.subtitle,
                self.theme.get(theming::Empty::Subtitle),
            ));
        }

        Paragraph::new(text)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}
