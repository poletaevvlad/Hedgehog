use crossterm::cursor::{MoveTo, RestorePosition, SavePosition};
use crossterm::style::{Print, SetBackgroundColor, SetForegroundColor};
use crossterm::QueueableCommand;
use std::{cell::RefCell, io::Stdout, rc::Rc};
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::style::{Color, Style};
use tui::widgets::Widget;

#[derive(Debug, Clone, Default)]
pub(crate) struct AnimationController {
    tick_count: usize,
    loading_indicators: Rc<RefCell<Vec<(u16, u16, Style)>>>,
}

impl AnimationController {
    pub(crate) fn clear(&mut self) {
        let mut indicators = self.loading_indicators.borrow_mut();
        indicators.clear();
    }

    pub(crate) fn add_loading_indicator(&self, x: u16, y: u16, style: Style) {
        let mut indicators = self.loading_indicators.borrow_mut();
        indicators.push((x, y, style));
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.loading_indicators.borrow().is_empty()
    }

    pub(crate) fn advance(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
    }

    pub(crate) fn render_loading_indicator(
        &self,
        stream: &mut Stdout,
        chars: &[char],
    ) -> crossterm::Result<()> {
        let frame_char = chars[self.tick_count % chars.len()];
        stream.queue(SavePosition)?;
        for (x, y, style) in self.loading_indicators.borrow().iter() {
            stream.queue(SetBackgroundColor(style.bg.unwrap_or(Color::Reset).into()))?;
            stream.queue(SetForegroundColor(style.fg.unwrap_or(Color::Reset).into()))?;
            stream.queue(MoveTo(*x, *y))?;
            stream.queue(Print(frame_char))?;
        }
        stream.queue(RestorePosition)?;
        Ok(())
    }
}

pub(crate) struct LoadingIndicator {
    controller: AnimationController,
    style: Style,
}

impl LoadingIndicator {
    pub(crate) fn new(controller: AnimationController) -> Self {
        LoadingIndicator {
            controller,
            style: Style::default(),
        }
    }

    pub(crate) fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl Widget for LoadingIndicator {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.controller
            .add_loading_indicator(area.x, area.y, self.style);
        buf.get_mut(area.x, area.y)
            .set_char('@')
            .set_style(self.style);
    }
}
