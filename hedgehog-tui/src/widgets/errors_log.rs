use super::{empty::EmptyView, errors_log_row::ErrorLogRowRenderer, list::List};
use crate::{logger::LogHistory, scrolling::ScrollableList, theming};
use tui::widgets::Widget;

pub struct ErrorsLogWidget<'a> {
    log: &'a ScrollableList<LogHistory>,
    theme: &'a theming::Theme,
}

impl<'a> ErrorsLogWidget<'a> {
    pub(crate) fn new(log: &'a ScrollableList<LogHistory>, theme: &'a theming::Theme) -> Self {
        ErrorsLogWidget { log, theme }
    }
}

impl<'a> Widget for ErrorsLogWidget<'a> {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        if self.log.data().is_empty() {
            EmptyView::new(self.theme)
                .title("The log is empty")
                .focused(true)
                .render(area, buf);
        } else {
            List::new(
                ErrorLogRowRenderer::new(self.theme),
                self.log.visible_iter(),
            )
            .item_height(3)
            .render(area, buf);
        }
    }
}
