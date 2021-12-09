use crate::status::Status;
use crate::theming::{self, Theme};
use tui::text::{Span, Spans};
use tui::widgets::{Paragraph, Widget};

pub(crate) struct StatusView<'a> {
    theme: &'a Theme,
    status: Option<&'a Status>,
}

impl<'a> StatusView<'a> {
    pub(crate) fn new(status: Option<&'a Status>, theme: &'a Theme) -> Self {
        StatusView { theme, status }
    }
}

impl<'a> Widget for StatusView<'a> {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        match self.status {
            Some(status) => {
                let mut spans = Vec::new();
                if let Some(label) = status.variant_label() {
                    let label_style = self
                        .theme
                        .get(theming::StatusBar::Status(Some(status.severity()), true));
                    spans.push(Span::styled(label, label_style));
                    spans.push(Span::styled(": ", label_style));
                }
                let style = self
                    .theme
                    .get(theming::StatusBar::Status(Some(status.severity()), false));
                spans.push(Span::raw(status.to_string()));
                let paragraph = Paragraph::new(Spans::from(spans)).style(style);
                paragraph.render(area, buf);
            }
            None => {
                buf.set_style(area, self.theme.get(theming::StatusBar::Empty));
            }
        }
    }
}
