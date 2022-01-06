use tui::style::Style;
use tui::widgets::Widget;

pub(crate) struct ProgressBar<'a> {
    style: Style,
    symbols: &'a [char],
    percentage: f64,
}

impl<'a> ProgressBar<'a> {
    pub(crate) fn new(percentage: f64) -> Self {
        ProgressBar {
            style: Style::default(),
            symbols: &[' ', '#'],
            percentage,
        }
    }

    pub(crate) fn symbols(mut self, symbols: &'a [char]) -> Self {
        self.symbols = symbols;
        self
    }

    pub(crate) fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl<'a> Widget for ProgressBar<'a> {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let states_count = area.width as usize * (self.symbols.len() - 1) + 1;
        let mut state = (states_count as f64 * self.percentage) as usize;

        for i in area.left()..area.right() {
            let cell = buf.get_mut(i, area.y);
            cell.set_style(self.style);
            cell.set_char(self.symbols[state.min(self.symbols.len() - 1)]);
            state = state.saturating_sub(self.symbols.len().saturating_sub(1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ProgressBar;
    use tui::backend::TestBackend;
    use tui::buffer::Buffer;
    use tui::layout::Rect;
    use tui::style::{Color, Style};
    use tui::Terminal;

    fn assert_display(width: u16, symbols: &[char], percentage: f64, expected: &str) {
        let backend = TestBackend::new(width, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let style = Style::default().fg(Color::Red);
        let widget = ProgressBar::new(percentage).symbols(symbols).style(style);
        terminal
            .draw(|frame| frame.render_widget(widget, frame.size()))
            .unwrap();

        let mut expected_buffer = Buffer::empty(Rect::new(0, 0, width, 1));
        expected_buffer.set_string(0, 0, expected, style);
        terminal.backend().assert_buffer(&expected_buffer);
    }

    fn assert_display_range(width: u16, symbols: &[char], p1: f64, p2: f64, expected: &str) {
        assert_display(width, symbols, p1, expected);
        assert_display(width, symbols, p2, expected);
    }

    #[test]
    fn two_symbols_two_width() {
        assert_display_range(2, &[' ', '#'], 0.00, 0.32, "  ");
        assert_display_range(2, &[' ', '#'], 0.34, 0.65, "# ");
        assert_display_range(2, &[' ', '#'], 0.67, 1.00, "##");
    }

    #[test]
    fn two_symbols_three_width() {
        assert_display_range(3, &[' ', '#'], 0.00, 0.24, "   ");
        assert_display_range(3, &[' ', '#'], 0.26, 0.49, "#  ");
        assert_display_range(3, &[' ', '#'], 0.51, 0.74, "## ");
        assert_display_range(3, &[' ', '#'], 0.76, 1.00, "###");
    }

    #[test]
    fn three_symbols_two_width() {
        assert_display_range(2, &[' ', 'X', '#'], 0.00, 0.19, "  ");
        assert_display_range(2, &[' ', 'X', '#'], 0.21, 0.39, "X ");
        assert_display_range(2, &[' ', 'X', '#'], 0.41, 0.59, "# ");
        assert_display_range(2, &[' ', 'X', '#'], 0.61, 0.79, "#X");
        assert_display_range(2, &[' ', 'X', '#'], 0.81, 1.00, "##");
    }

    #[test]
    fn three_symbols_three_width() {
        assert_display_range(3, &[' ', 'X', '#'], 0.00, 1.0 / 7.0 - 0.1, "   ");
        assert_display_range(3, &[' ', 'X', '#'], 1.0 / 7.0 + 0.1, 2.0 / 7.0 - 0.1, "X  ");
        assert_display_range(3, &[' ', 'X', '#'], 2.0 / 7.0 + 0.1, 3.0 / 7.0 - 0.1, "#  ");
        assert_display_range(3, &[' ', 'X', '#'], 3.0 / 7.0 + 0.1, 4.0 / 7.0 - 0.1, "#X ");
        assert_display_range(3, &[' ', 'X', '#'], 4.0 / 7.0 + 0.1, 5.0 / 7.0 - 0.1, "## ");
        assert_display_range(3, &[' ', 'X', '#'], 5.0 / 7.0 + 0.1, 6.0 / 7.0 - 0.1, "##X");
        assert_display_range(3, &[' ', 'X', '#'], 6.0 / 7.0 + 0.1, 1.0, "###");
    }
}
