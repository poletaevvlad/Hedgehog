use crate::command::CommandConfirmation;
use crate::theming::{self, Theme};
use tui::widgets::{Paragraph, Widget};

pub(crate) struct ConfirmationView<'a> {
    theme: &'a Theme,
    confirmation: &'a CommandConfirmation,
}

impl<'a> ConfirmationView<'a> {
    pub(crate) fn new(confirmation: &'a CommandConfirmation, theme: &'a Theme) -> Self {
        ConfirmationView {
            confirmation,
            theme,
        }
    }
}

impl<'a> Widget for ConfirmationView<'a> {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let text = format!(
            "{} [{}/{}]",
            self.confirmation.prompt,
            if self.confirmation.default { 'Y' } else { 'y' },
            if self.confirmation.default { 'n' } else { 'N' },
        );
        Paragraph::new(text)
            .style(self.theme.get(theming::StatusBar::Confirmation))
            .render(area, buf);
    }
}
