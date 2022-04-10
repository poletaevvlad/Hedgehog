use crate::logger::LogEntry;
use crate::theming::{self, Theme};
use tui::text::{Span, Spans};
use tui::widgets::{Paragraph, Widget};

pub(crate) struct LogEntryView<'a> {
    theme: &'a Theme,
    log_entry: Option<&'a LogEntry>,
}

impl<'a> LogEntryView<'a> {
    pub(crate) fn new(log_entry: Option<&'a LogEntry>, theme: &'a Theme) -> Self {
        LogEntryView { theme, log_entry }
    }
}

impl<'a> Widget for LogEntryView<'a> {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        match self.log_entry {
            Some(log_entry) => {
                let mut spans = Vec::new();
                if let Some(label) = log_entry.variant_label() {
                    let label_style = self
                        .theme
                        .get(theming::StatusBar::Status(Some(log_entry.severity()), true));
                    spans.push(Span::styled(label, label_style));
                    spans.push(Span::styled(": ", label_style));
                }
                let style = self.theme.get(theming::StatusBar::Status(
                    Some(log_entry.severity()),
                    false,
                ));
                spans.push(Span::raw(log_entry.message()));
                let paragraph = Paragraph::new(Spans::from(spans)).style(style);
                paragraph.render(area, buf);
            }
            None => {
                buf.set_style(area, self.theme.get(theming::StatusBar::Empty));
            }
        }
    }
}
