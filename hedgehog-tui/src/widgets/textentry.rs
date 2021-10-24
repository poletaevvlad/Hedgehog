use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use tui::backend::Backend;
use tui::buffer;
use tui::layout::Rect;
use tui::style::Style;
use tui::text::Span;
use tui::widgets::Widget;
use tui::Frame;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::events::key;

pub(crate) enum Direction {
    Forward,
    Backward,
}

pub(crate) enum Amount {
    Character,
    All,
    Word,
}

#[derive(Debug)]
pub(crate) struct Buffer {
    text: String,
    cursor_position: usize,
    display_offset: u16,
}

impl Buffer {
    #[cfg(test)]
    pub(crate) fn new(text: String, cursor_position: usize) -> Buffer {
        Buffer {
            text,
            cursor_position,
            display_offset: 0,
        }
    }

    pub(crate) fn push_char(&mut self, ch: char) {
        self.text.insert(self.cursor_position, ch);
        self.cursor_position += ch.len_utf8();
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.text
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn cursor_position(&self) -> usize {
        self.cursor_position
    }

    fn char_at(&self, index: usize) -> Option<char> {
        self.text[index..].chars().next()
    }

    fn go_backward(&self, amount: Amount) -> (usize, bool) {
        if self.cursor_position == 0 {
            return (0, true);
        }

        fn advance(text: &str, mut index: usize) -> usize {
            index -= 1;
            while !text.is_char_boundary(index) {
                index -= 1;
            }
            index
        }

        match amount {
            Amount::Character => (advance(&self.text, self.cursor_position), false),
            Amount::Word => {
                let mut index = advance(&self.text, self.cursor_position);
                // Skipping whitespaces
                while index > 0 {
                    if let Some(false) = self.char_at(index).map(char::is_whitespace) {
                        break;
                    }
                    index = advance(&self.text, index);
                }

                // Finding the first whitespace after the word or end of string
                while index > 0 {
                    let next = advance(&self.text, index);
                    if self.char_at(next).unwrap().is_whitespace() {
                        break;
                    }
                    index = next
                }
                (index, index == 0)
            }
            Amount::All => (0, true),
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
                while let Some(true) = self.char_at(index).map(char::is_whitespace) {
                    index = advance(&self.text, index);
                }

                // Finding the first whitespace after the word or end of string
                while let Some(false) = self.char_at(index).map(char::is_whitespace) {
                    index = advance(&self.text, index);
                }
                index
            }
            Amount::All => max_position,
        }
    }

    pub(crate) fn move_cursor(&mut self, direction: Direction, amount: Amount) -> bool {
        let (new_position, reset_offset) = match direction {
            Direction::Backward => self.go_backward(amount),
            Direction::Forward => (self.go_forward(amount), false),
        };

        let mut changed = false;
        if new_position != self.cursor_position {
            self.cursor_position = new_position;
            changed = true;
        }
        if reset_offset && self.display_offset != 0 {
            self.display_offset = 0;
            changed = true;
        }
        changed
    }

    pub(crate) fn delete(&mut self, direction: Direction, amount: Amount) -> bool {
        match direction {
            Direction::Backward if self.cursor_position > 0 => {
                let new_index = self.go_backward(amount).0;
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

    pub(crate) fn is_editing_event(event: Event) -> bool {
        matches!(
            event,
            Event::Key(KeyEvent {
                code: KeyCode::Char(_),
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
            }) | key!(Left)
                | key!(Left, CONTROL)
                | key!(Right)
                | key!(Right, CONTROL)
                | key!(Home)
                | key!(End)
                | key!(Backspace)
                | key!(Backspace, SHIFT)
                | key!(Backspace, ALT)
                | key!(Delete)
                | key!(Delete, SHIFT)
                | key!(Delete, CONTROL)
                | key!('w', CONTROL)
        )
    }

    pub(crate) fn handle_event(&mut self, event: Event) -> bool {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Char(ch),
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
            }) => {
                self.push_char(ch);
                true
            }
            key!(Left) => self.move_cursor(Direction::Backward, Amount::Character),
            key!(Right) => self.move_cursor(Direction::Forward, Amount::Character),
            key!(Left, CONTROL) => self.move_cursor(Direction::Backward, Amount::Word),
            key!(Right, CONTROL) => self.move_cursor(Direction::Forward, Amount::Word),
            key!(Home) => self.move_cursor(Direction::Backward, Amount::All),
            key!(End) => self.move_cursor(Direction::Forward, Amount::All),
            key!(Backspace) => self.delete(Direction::Backward, Amount::Character),
            key!(Delete) => self.delete(Direction::Forward, Amount::Character),
            key!(Backspace, SHIFT) => self.delete(Direction::Backward, Amount::All),
            key!(Delete, SHIFT) => self.delete(Direction::Forward, Amount::All),
            key!(Backspace, ALT) | key!('w', CONTROL) => {
                self.delete(Direction::Backward, Amount::Word)
            }
            key!(Delete, CONTROL) => self.delete(Direction::Forward, Amount::Word),
            _ => false,
        }
    }
}

impl From<String> for Buffer {
    fn from(text: String) -> Self {
        Buffer {
            cursor_position: text.len(),
            text,
            display_offset: 0,
        }
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Buffer {
            text: String::new(),
            cursor_position: 0,
            display_offset: 0,
        }
    }
}

#[derive(Default)]
pub(crate) struct Entry<'a> {
    prefix: Option<Span<'a>>,
    style: Style,
}

impl<'a> Entry<'a> {
    pub(crate) fn new() -> Self {
        Entry::default()
    }

    pub(crate) fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub(crate) fn prefix(mut self, prefix: impl Into<Option<Span<'a>>>) -> Self {
        self.prefix = prefix.into();
        self
    }

    fn prefix_width(&self) -> usize {
        self.prefix.as_ref().map(Span::width).unwrap_or(0)
    }

    pub(crate) fn render<B: Backend>(self, f: &mut Frame<B>, area: Rect, state: &mut Buffer) {
        let width_before_cursor = self.prefix_width() + state.text[..state.cursor_position].width();
        let width_after_cursor = state.text[state.cursor_position..].width();
        let max_cursor_position = area.width as i32 - 1;

        let mut cursor_position = width_before_cursor as i32 - state.display_offset as i32;
        if cursor_position < 0 {
            state.display_offset -= (-cursor_position) as u16;
            cursor_position = 0;
        } else if cursor_position > max_cursor_position as i32 {
            let delta = cursor_position as i32 - max_cursor_position as i32;
            state.display_offset += delta as u16;
            cursor_position = max_cursor_position;
        }

        let empty_space_after =
            area.width as i32 - cursor_position as i32 - width_after_cursor as i32 - 1;
        if empty_space_after > 0 {
            state.display_offset = state
                .display_offset
                .saturating_sub(empty_space_after as u16);
            cursor_position = width_before_cursor as i32 - state.display_offset as i32;
        }

        let widget = TextEntryWidget {
            prefix: self.prefix,
            display_offset: state.display_offset,
            text: state.text.as_str(),
            style: self.style,
        };
        f.render_widget(widget, area);
        f.set_cursor(area.x + cursor_position as u16, area.y);
    }
}

struct TextEntryWidget<'a> {
    prefix: Option<Span<'a>>,
    text: &'a str,
    display_offset: u16,
    style: Style,
}

fn skip_by_width(mut offset: u16, mut text: &str) -> (u16, &str) {
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        let width = ch.width().unwrap_or(0) as u16;
        if width == 0 || offset > 0 {
            offset = offset.saturating_sub(width);
            text = chars.as_str();
        } else {
            break;
        }
    }
    (offset, text)
}

impl<'a> Widget for TextEntryWidget<'a> {
    fn render(self, mut area: Rect, buf: &mut buffer::Buffer) {
        buf.set_style(area, self.style);

        let mut remaining_offset = self.display_offset as u16;
        if let Some(ref prefix) = self.prefix {
            let (remaining, prefix_text) = skip_by_width(remaining_offset, &prefix.content);
            let (x, _) = buf.set_span(
                area.left(),
                area.top(),
                &Span::styled(prefix_text, prefix.style),
                area.width,
            );
            area = Rect::new(x, area.y, area.width - (x - area.x), area.height);
            remaining_offset = remaining;
        }

        let (_, text) = skip_by_width(remaining_offset, self.text);
        buf.set_span(area.left(), area.top(), &Span::raw(text), area.width);
    }
}

pub(crate) struct ReadonlyEntry<'a> {
    prefix: Option<Span<'a>>,
    text: &'a str,
    style: Style,
}

impl<'a> ReadonlyEntry<'a> {
    pub(crate) fn new(text: &'a str) -> Self {
        ReadonlyEntry {
            prefix: None,
            text,
            style: Style::default(),
        }
    }

    pub(crate) fn prefix(mut self, prefix: impl Into<Option<Span<'a>>>) -> Self {
        self.prefix = prefix.into();
        self
    }

    pub(crate) fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub(crate) fn render<B: Backend>(self, f: &mut Frame<B>, area: Rect) {
        let width_before_cursor =
            self.prefix.as_ref().map(Span::width).unwrap_or(0) as usize + self.text.width();
        let max_cursor_position = area.width as i32 - 1;

        let mut cursor_position = width_before_cursor as i32;
        let mut display_offset = 0;
        if cursor_position > max_cursor_position as i32 {
            display_offset = (cursor_position as i32 - max_cursor_position as i32) as u16;
            cursor_position = max_cursor_position;
        }

        let widget = TextEntryWidget {
            prefix: self.prefix,
            display_offset,
            text: self.text,
            style: self.style,
        };
        f.render_widget(widget, area);
        f.set_cursor(area.x + cursor_position as u16, area.y);
    }
}

#[cfg(test)]
mod tests {
    use super::{Amount, Buffer, Direction, Entry, ReadonlyEntry};

    mod buffer {
        use super::*;

        fn assert_buffer(buffer: &Buffer, text: &str, position: usize) {
            assert_eq!(buffer.as_str(), text);
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

        #[test]
        fn deletion_word() {
            let mut buffer = Buffer::new("каждый охотник желает...".to_string(), 0);
            assert!(buffer.delete(Direction::Forward, Amount::Word));
            assert_eq!(buffer.as_str(), " охотник желает...");
            assert!(buffer.delete(Direction::Forward, Amount::Word));
            assert_eq!(buffer.as_str(), " желает...");
            assert!(buffer.delete(Direction::Forward, Amount::Word));
            assert_eq!(buffer.as_str(), "");
            assert!(!buffer.delete(Direction::Forward, Amount::Word));

            let mut buffer = Buffer::from("каждый охотник желает...".to_string());
            assert!(buffer.delete(Direction::Backward, Amount::Word));
            assert_eq!(buffer.as_str(), "каждый охотник ");
            assert!(buffer.delete(Direction::Backward, Amount::Word));
            assert_eq!(buffer.as_str(), "каждый ");
            assert!(buffer.delete(Direction::Backward, Amount::Word));
            assert_eq!(buffer.as_str(), "");
            assert!(!buffer.delete(Direction::Forward, Amount::Word));
        }
    }

    mod widget {
        use super::*;
        use crate::events::key;
        use crossterm::event::Event;
        use tui::backend::{Backend, TestBackend};
        use tui::buffer::Buffer as TuiBuffer;
        use tui::style::Style;
        use tui::text::Span;
        use tui::Terminal;

        fn draw_entry(terminal: &mut Terminal<impl Backend>, buffer: &mut Buffer) {
            terminal
                .draw(|f| {
                    Entry::new()
                        .prefix(Span::raw("::"))
                        .render(f, f.size(), buffer)
                })
                .unwrap();
        }

        struct BufferTester {
            buffer: Buffer,
            terminal: Terminal<TestBackend>,
        }

        impl BufferTester {
            fn new(buffer: Buffer, width: u16) -> Self {
                let backend = TestBackend::new(width, 1);
                let terminal = Terminal::new(backend).unwrap();
                BufferTester { buffer, terminal }
            }

            fn assert_response(&mut self, event: Event, expected: &str, cursor: (u16, u16)) {
                self.buffer.handle_event(event);
                draw_entry(&mut self.terminal, &mut self.buffer);
                let size = self.terminal.size().unwrap();
                let mut buffer = TuiBuffer::empty(size);
                buffer.set_string(0, 0, expected, Style::default());
                self.terminal.backend().assert_buffer(&buffer);
                assert_eq!(self.terminal.backend_mut().get_cursor().unwrap(), cursor);
            }
        }

        #[test]
        fn scrolling_text() {
            let mut tester = BufferTester::new(Buffer::default(), 5);
            tester.assert_response(key!(Up), "::", (2, 0));
            tester.assert_response(key!('a'), "::a", (3, 0));
            tester.assert_response(key!('b'), "::ab", (4, 0));
            tester.assert_response(key!('c'), ":abc", (4, 0));
            tester.assert_response(key!('d'), "abcd", (4, 0));
            tester.assert_response(key!('e'), "bcde", (4, 0));
            tester.assert_response(key!(Left), "bcde", (3, 0));
            tester.assert_response(key!(Left), "bcde", (2, 0));
            tester.assert_response(key!(Left), "bcde", (1, 0));
            tester.assert_response(key!(Left), "bcde", (0, 0));
            tester.assert_response(key!(Left), "abcde", (0, 0));
            tester.assert_response(key!(Left), "::abc", (2, 0));
            tester.assert_response(key!(End), "bcde", (4, 0));
            tester.assert_response(key!(Home), "::abc", (2, 0));
        }

        #[test]
        fn wide_character() {
            let mut tester = BufferTester::new(Buffer::default(), 6);
            tester.assert_response(key!('ア'), "::ア", (4, 0));
            tester.assert_response(key!('イ'), ":アイ", (5, 0));
            tester.assert_response(key!('-'), "アイ-", (5, 0));
            tester.assert_response(key!('-'), "イ--", (5, 0));
            tester.assert_response(key!('あ'), "--あ", (5, 0));
            tester.assert_response(key!('お'), "-あお", (5, 0));
        }
    }

    mod readonly_widget {
        use super::*;
        use tui::backend::{Backend, TestBackend};
        use tui::buffer::Buffer as TuiBuffer;
        use tui::layout::Rect;
        use tui::style::Style;
        use tui::text::Span;
        use tui::Terminal;

        fn assert_readonly(width: u16, text: &str, expected: &str, cursor: (u16, u16)) {
            let backend = TestBackend::new(width, 1);
            let mut terminal = Terminal::new(backend).unwrap();
            terminal
                .draw(|f| {
                    ReadonlyEntry::new(text)
                        .prefix(Span::raw("::"))
                        .render(f, Rect::new(0, 0, width, 1))
                })
                .unwrap();

            let mut buffer = TuiBuffer::empty(Rect::new(0, 0, width, 1));
            buffer.set_string(0, 0, expected, Style::default());
            terminal.backend().assert_buffer(&buffer);
            assert_eq!(terminal.backend_mut().get_cursor().unwrap(), cursor);
        }

        #[test]
        fn displaying_readonly() {
            assert_readonly(5, "", "::", (2, 0));
            assert_readonly(5, "a", "::a", (3, 0));
            assert_readonly(5, "ab", "::ab", (4, 0));
            assert_readonly(5, "abc", ":abc", (4, 0));
            assert_readonly(5, "abcdef", "cdef", (4, 0));
        }

        #[test]
        fn displaying_wide_characters() {
            assert_readonly(6, "あ", "::あ", (4, 0));
            assert_readonly(6, "あえ", ":あえ", (5, 0));
            assert_readonly(6, "あえ-", "あえ-", (5, 0));
            assert_readonly(6, "あえ--", "え--", (5, 0));
            assert_readonly(6, "あえ--あ", "--あ", (5, 0));
            assert_readonly(6, "あえ--あお", "-あお", (5, 0));
        }
    }
}
