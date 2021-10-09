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
    Word,
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
        self.cursor_position += ch.len_utf8();
    }

    pub(crate) fn as_slice(&self) -> &str {
        &self.text
    }

    pub(crate) fn cursor_position(&self) -> usize {
        self.cursor_position
    }

    fn go_backward(&self, amount: Amount) -> usize {
        if self.cursor_position == 0 {
            return 0;
        }

        fn advance(text: &str, mut index: usize) -> usize {
            index -= 1;
            while !text.is_char_boundary(index) {
                index -= 1;
            }
            index
        }

        match amount {
            Amount::Character => advance(&self.text, self.cursor_position),
            Amount::Word => {
                let mut index = advance(&self.text, self.cursor_position);
                while index > 0 {
                    if let Some(false) = self.text[index..].chars().next().map(char::is_whitespace)
                    {
                        break;
                    }
                    index = advance(&self.text, index);
                }
                while index > 0 {
                    let next = advance(&self.text, index);
                    if self.text[next..].chars().next().unwrap().is_whitespace() {
                        break;
                    }
                    index = next
                }
                index
            }
            Amount::All => 0,
        }
    }

    fn go_forward(&self, amount: Amount) -> usize {
        let max_position = self.text.len();
        if self.cursor_position >= max_position {
            return max_position;
        }

        fn advance(text: &str, mut index: usize) -> usize {
            index += 1;
            while !text.is_char_boundary(index) {
                index += 1;
            }
            index
        }

        match amount {
            Amount::Character => advance(&self.text, self.cursor_position),
            Amount::Word => {
                let mut index = self.cursor_position;
                // Skipping whitespaces
                while let Some(true) = self.text[index..].chars().next().map(char::is_whitespace) {
                    index = advance(&self.text, index);
                }

                //Finding the first whitespace after the word or end of string
                while let Some(false) = self.text[index..].chars().next().map(char::is_whitespace) {
                    index = advance(&self.text, index);
                }
                index
            }
            Amount::All => max_position,
        }
    }

    pub(crate) fn move_cursor(&mut self, direction: Direction, amount: Amount) -> bool {
        let new_position = match direction {
            Direction::Backward => self.go_backward(amount),
            Direction::Forward => self.go_forward(amount),
        };
        if new_position != self.cursor_position {
            self.cursor_position = new_position;
            true
        } else {
            false
        }
    }

    pub(crate) fn delete(&mut self, direction: Direction, amount: Amount) -> bool {
        match direction {
            Direction::Backward if self.cursor_position > 0 => {
                let new_index = self.go_backward(amount);
                self.text.drain(new_index..self.cursor_position);
                self.cursor_position = new_index;
                true
            }
            Direction::Forward if self.cursor_position < self.text.len() => {
                let new_index = self.go_forward(amount);
                self.text.drain(self.cursor_position..new_index);
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
            area.x
                + self.prefix_width()
                + Span::raw(&state.text[0..state.cursor_position]).width() as u16,
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
            let mut buffer = Buffer::from("аб".to_string()); // cyrilic, 2 bytes per character
            assert_buffer(&buffer, "аб", 4);

            assert!(!buffer.move_cursor(Direction::Forward, Amount::Character));
            assert_eq!(buffer.cursor_position(), 4);

            assert!(buffer.move_cursor(Direction::Backward, Amount::Character));
            assert_eq!(buffer.cursor_position(), 2);

            assert!(buffer.move_cursor(Direction::Backward, Amount::Character));
            assert_eq!(buffer.cursor_position(), 0);

            assert!(!buffer.move_cursor(Direction::Backward, Amount::Character));
            assert_eq!(buffer.cursor_position(), 0);

            assert!(buffer.move_cursor(Direction::Forward, Amount::Character));
            assert_eq!(buffer.cursor_position(), 2);
        }

        #[test]
        fn movement_all() {
            let mut buffer = Buffer::from("абвг".to_string());

            assert!(!buffer.move_cursor(Direction::Forward, Amount::All));
            assert_eq!(buffer.cursor_position(), 8);
            assert!(buffer.move_cursor(Direction::Backward, Amount::All));
            assert_eq!(buffer.cursor_position(), 0);

            assert!(!buffer.move_cursor(Direction::Backward, Amount::All));
            assert_eq!(buffer.cursor_position(), 0);
            assert!(buffer.move_cursor(Direction::Forward, Amount::All));
            assert_eq!(buffer.cursor_position(), 8);
        }

        #[test]
        fn movement_word() {
            let mut buffer = Buffer::new("каждый охотник желает...".to_string(), 0);
            assert_eq!(buffer.cursor_position(), 0);

            assert!(buffer.move_cursor(Direction::Forward, Amount::Word));
            assert_eq!(buffer.cursor_position(), 12); // каждый| охотник

            assert!(buffer.move_cursor(Direction::Forward, Amount::Word));
            assert_eq!(buffer.cursor_position(), 27); // каждый охотник| желает

            assert!(buffer.move_cursor(Direction::Forward, Amount::Word));
            assert_eq!(buffer.cursor_position(), 43);

            assert!(!buffer.move_cursor(Direction::Forward, Amount::Word));
            assert_eq!(buffer.cursor_position(), 43);

            assert!(buffer.move_cursor(Direction::Backward, Amount::Word));
            assert_eq!(buffer.cursor_position(), 28);

            assert!(buffer.move_cursor(Direction::Backward, Amount::Word));
            assert_eq!(buffer.cursor_position(), 13);

            assert!(buffer.move_cursor(Direction::Backward, Amount::Word));
            assert_eq!(buffer.cursor_position(), 0);

            assert!(!buffer.move_cursor(Direction::Backward, Amount::Word));
            assert_eq!(buffer.cursor_position(), 0);
        }

        #[test]
        fn movement_word_only_spaces() {
            let mut buffer = Buffer::new("   ".to_string(), 0);
            assert!(buffer.move_cursor(Direction::Forward, Amount::Word));
            assert_eq!(buffer.cursor_position(), 3);

            assert!(buffer.move_cursor(Direction::Backward, Amount::Word));
            assert_eq!(buffer.cursor_position(), 0);
        }

        #[test]
        fn movement_empty() {
            let mut buffer = Buffer::default();

            assert!(!buffer.move_cursor(Direction::Forward, Amount::Character));
            assert!(!buffer.move_cursor(Direction::Forward, Amount::All));
            assert!(!buffer.move_cursor(Direction::Forward, Amount::Word));
            assert!(!buffer.move_cursor(Direction::Backward, Amount::Character));
            assert!(!buffer.move_cursor(Direction::Backward, Amount::All));
            assert!(!buffer.move_cursor(Direction::Backward, Amount::Word));
        }

        #[test]
        fn deletion_character() {
            let mut buffer = Buffer::from("абв".to_string());
            assert!(!buffer.delete(Direction::Forward, Amount::Character));
            assert_buffer(&buffer, "абв", 6);
            assert!(buffer.delete(Direction::Backward, Amount::Character));
            assert_buffer(&buffer, "аб", 4);

            let mut buffer = Buffer::new("абв".to_string(), 0);
            assert!(buffer.delete(Direction::Forward, Amount::Character));
            assert_buffer(&buffer, "бв", 0);
            assert!(!buffer.delete(Direction::Backward, Amount::Character));
            assert_buffer(&buffer, "бв", 0);

            let mut buffer = Buffer::new("абгд".to_string(), 4);
            assert!(buffer.delete(Direction::Forward, Amount::Character));
            assert_buffer(&buffer, "абд", 4);
            assert!(buffer.delete(Direction::Backward, Amount::Character));
            assert_buffer(&buffer, "ад", 2);
        }

        #[test]
        fn deletion_all() {
            let mut buffer = Buffer::new("абвг".to_string(), 4);
            assert!(buffer.delete(Direction::Forward, Amount::All));
            assert_buffer(&buffer, "аб", 4);

            let mut buffer = Buffer::new("абвг".to_string(), 4);
            assert!(buffer.delete(Direction::Backward, Amount::All));
            assert_buffer(&buffer, "вг", 0);

            let mut buffer = Buffer::new("абвг".to_string(), 0);
            assert!(!buffer.delete(Direction::Backward, Amount::All));
            assert_buffer(&buffer, "абвг", 0);

            let mut buffer = Buffer::new("абвг".to_string(), 8);
            assert!(!buffer.delete(Direction::Forward, Amount::All));
            assert_buffer(&buffer, "абвг", 8);
        }
    }
}
