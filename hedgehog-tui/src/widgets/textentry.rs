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

pub(crate) enum Amount {
    Character,
    All,
}

pub(crate) struct Buffer {
    text: String,
    cursor_position: usize,
}

impl Buffer {
    pub(crate) fn new(text: String, cursor_position: usize) -> Buffer {
        Buffer {
            text,
            cursor_position,
        }
    }

    pub(crate) fn push_char(&mut self, ch: char) {
        self.text.insert(self.cursor_position, ch);
        self.cursor_position += 1;
    }

    pub(crate) fn as_slice(&self) -> &str {
        &self.text
    }

    pub(crate) fn cursor_position(&self) -> usize {
        self.cursor_position
    }

    pub(crate) fn move_cursor(&mut self, direction: Direction, amount: Amount) -> bool {
        match direction {
            Direction::Backward if self.cursor_position > 0 => {
                match amount {
                    Amount::Character => self.cursor_position -= 1,
                    Amount::All => self.cursor_position = 0,
                }
                true
            }
            Direction::Forward if self.cursor_position < self.text.len() => {
                match amount {
                    Amount::Character => self.cursor_position += 1,
                    Amount::All => self.cursor_position = self.text.len(),
                }
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

impl From<String> for Buffer {
    fn from(text: String) -> Self {
        Buffer {
            cursor_position: text.len(),
            text,
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

    fn prefix_width(&self) -> u16 {
        self.prefix.as_ref().map(Span::width).unwrap_or(0) as u16
    }

    pub(crate) fn render<B: Backend>(self, f: &mut Frame<B>, area: Rect, state: &mut Buffer) {
        f.set_cursor(
            area.x + self.prefix_width() + state.cursor_position as u16,
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

#[cfg(test)]
mod tests {
    use super::{Amount, Buffer, Direction};

    mod buffer {
        use super::*;

        fn assert_buffer(buffer: &Buffer, text: &str, position: usize) {
            assert_eq!(buffer.as_slice(), text);
            assert_eq!(buffer.cursor_position(), position);
        }

        #[test]
        fn movement_character() {
            let mut buffer = Buffer::from("ab".to_string());
            assert_buffer(&buffer, "ab", 2);

            assert!(!buffer.move_cursor(Direction::Forward, Amount::Character));
            assert_eq!(buffer.cursor_position(), 2);

            assert!(buffer.move_cursor(Direction::Backward, Amount::Character));
            assert_eq!(buffer.cursor_position(), 1);

            assert!(buffer.move_cursor(Direction::Backward, Amount::Character));
            assert_eq!(buffer.cursor_position(), 0);

            assert!(!buffer.move_cursor(Direction::Backward, Amount::Character));
            assert_eq!(buffer.cursor_position(), 0);

            assert!(buffer.move_cursor(Direction::Forward, Amount::Character));
            assert_eq!(buffer.cursor_position(), 1);
        }

        #[test]
        fn movement_all() {
            let mut buffer = Buffer::from("abcd".to_string());

            assert!(!buffer.move_cursor(Direction::Forward, Amount::All));
            assert_eq!(buffer.cursor_position(), 4);
            assert!(buffer.move_cursor(Direction::Backward, Amount::All));
            assert_eq!(buffer.cursor_position(), 0);

            assert!(!buffer.move_cursor(Direction::Backward, Amount::All));
            assert_eq!(buffer.cursor_position(), 0);
            assert!(buffer.move_cursor(Direction::Forward, Amount::All));
            assert_eq!(buffer.cursor_position(), 4);
        }

        #[test]
        fn character_deletion() {
            let mut buffer = Buffer::from("abc".to_string());
            assert!(!buffer.delete_char(Direction::Forward));
            assert_buffer(&buffer, "abc", 3);
            assert!(buffer.delete_char(Direction::Backward));
            assert_buffer(&buffer, "ab", 2);

            let mut buffer = Buffer::new("abc".to_string(), 0);
            assert!(buffer.delete_char(Direction::Forward));
            assert_buffer(&buffer, "bc", 0);
            assert!(!buffer.delete_char(Direction::Backward));
            assert_buffer(&buffer, "bc", 0);

            let mut buffer = Buffer::new("abcd".to_string(), 2);
            assert!(buffer.delete_char(Direction::Forward));
            assert_buffer(&buffer, "abd", 2);
            assert!(buffer.delete_char(Direction::Backward));
            assert_buffer(&buffer, "ad", 1);
        }
    }
}
