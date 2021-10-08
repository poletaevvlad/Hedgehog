use tui::backend::Backend;
use tui::buffer;
use tui::layout::Rect;
use tui::text::Span;
use tui::widgets::StatefulWidget;
use tui::Frame;

pub(crate) enum Direction {
    Forward,
    Backward,
}

pub(crate) struct Buffer {
    text: String,
    cursor_position: usize,
}

impl Buffer {
    pub(crate) fn push_char(&mut self, ch: char) {
        self.text.insert(self.cursor_position, ch);
        self.cursor_position += 1;
    }

    pub(crate) fn move_cursor(&mut self, direction: Direction) -> bool {
        match direction {
            Direction::Backward if self.cursor_position > 0 => {
                self.cursor_position -= 1;
                true
            }
            Direction::Forward if self.cursor_position < self.text.len() => {
                self.cursor_position += 1;
                true
            }
            _ => false,
        }
    }

    pub(crate) fn delete_char(&mut self, direction: Direction) -> bool {
        match direction {
            Direction::Backward if self.cursor_position > 0 => {
                self.text.remove(self.cursor_position - 1);
                self.cursor_position -= 1;
                true
            }
            Direction::Forward if self.cursor_position < self.text.len() => {
                self.text.remove(self.cursor_position);
                true
            }
            _ => false,
        }
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Buffer {
            text: String::new(),
            cursor_position: 0,
        }
    }
}

#[derive(Default)]
pub(crate) struct Entry<'a> {
    prefix: Option<Span<'a>>,
}

impl<'a> Entry<'a> {
    pub(crate) fn new() -> Self {
        Entry::default()
    }

    pub(crate) fn prefix(mut self, prefix: Span<'a>) -> Self {
        self.prefix = Some(prefix);
        self
    }

    pub(crate) fn render<B: Backend>(self, f: &mut Frame<B>, area: Rect, state: &mut Buffer) {
        f.set_cursor(
            area.x
                + self
                    .prefix
                    .as_ref()
                    .map(|prefix| prefix.width())
                    .unwrap_or(0) as u16
                + state.cursor_position as u16,
            area.y,
        );
        f.render_stateful_widget(
            TextEntryWidget {
                prefix: self.prefix,
            },
            area,
            state,
        );
    }
}

struct TextEntryWidget<'a> {
    prefix: Option<Span<'a>>,
}

impl<'a> StatefulWidget for TextEntryWidget<'a> {
    type State = Buffer;

    fn render(self, mut area: Rect, buf: &mut buffer::Buffer, state: &mut Self::State) {
        if let Some(ref prefix) = self.prefix {
            let (x, _) = buf.set_span(area.left(), area.top(), prefix, area.width);
            area = Rect::new(x, area.y, area.width - (x - area.x), area.height);
        }
        buf.set_span(area.left(), area.top(), &Span::raw(&state.text), area.width);
    }
}
