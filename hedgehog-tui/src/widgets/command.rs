use super::textentry::{Buffer, Entry, ReadonlyEntry};
use crate::events::key;
use crate::history::CommandsHistory;
use crate::theming::{self, Theme};
use crossterm::event::Event;
use tui::backend::Backend;
use tui::layout::Rect;
use tui::style::Style;
use tui::text::Span;
use tui::Frame;

#[derive(Debug)]
pub(crate) struct CommandState {
    buffer: Buffer,
    history_index: Option<usize>,
}

#[derive(Debug, PartialEq)]
pub(crate) enum CommandActionResult {
    None,
    Update,
    Clear,
    Submit,
}

impl CommandState {
    pub(crate) fn as_str<'a>(&'a self, history: &'a CommandsHistory) -> &'a str {
        self.history_index
            .and_then(|index| history.get(index))
            .unwrap_or_else(|| self.buffer.as_str())
    }

    pub(crate) fn handle_event(
        &mut self,
        event: Event,
        history: &CommandsHistory,
    ) -> CommandActionResult {
        match event {
            key!('c', CONTROL) | key!(Esc) => CommandActionResult::Clear,
            key!(Backspace) if self.buffer.is_empty() && self.history_index.is_none() => {
                CommandActionResult::Clear
            }
            key!(Up) | key!(Down) => {
                let history_index = self.history_index.as_ref();
                let found_index = match event {
                    key!(Up) => {
                        let history_index = history_index.map(|index| index + 1).unwrap_or(0);
                        history.find_before(history_index, self.buffer.as_str())
                    }
                    key!(Down) if history_index != Some(&0) => {
                        let history_index = history_index.map(|index| index - 1).unwrap_or(0);
                        history.find_after(history_index, self.buffer.as_str())
                    }
                    _ => None,
                };

                match found_index {
                    None if event == key!(Down) => {
                        self.history_index = None;
                        CommandActionResult::Update
                    }
                    None => CommandActionResult::None,
                    Some(new_index) => {
                        self.history_index = Some(new_index);
                        CommandActionResult::Update
                    }
                }
            }
            key!(Enter) if self.buffer.is_empty() && self.history_index.is_none() => {
                CommandActionResult::Clear
            }
            key!(Enter) => CommandActionResult::Submit,
            event if Buffer::is_editing_event(event) => {
                let history_str = self.history_index.and_then(|index| history.get(index));
                if let Some(string) = history_str {
                    self.buffer = Buffer::from(string.to_string());
                    self.history_index = None;
                }
                match self.buffer.handle_event(event) {
                    true => CommandActionResult::Update,
                    false => CommandActionResult::None,
                }
            }
            _ => CommandActionResult::None,
        }
    }
}

impl Default for CommandState {
    fn default() -> Self {
        CommandState {
            buffer: Buffer::default(),
            history_index: None,
        }
    }
}

pub(crate) struct CommandEditor<'a> {
    state: &'a mut CommandState,
    prefix: Option<Span<'a>>,
    style: Style,
}

impl<'a> CommandEditor<'a> {
    pub(crate) fn new(state: &'a mut CommandState) -> Self {
        CommandEditor {
            state,
            prefix: None,
            style: Style::default(),
        }
    }

    pub(crate) fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub(crate) fn prefix(mut self, prefix: impl Into<Span<'a>>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    pub(crate) fn theme(mut self, theme: &Theme) -> Self {
        self = self.style(theme.get(theming::StatusBar::Command));
        if let Some(ref mut prefix) = self.prefix {
            prefix.style = theme.get(theming::StatusBar::CommandPrompt);
        }
        self
    }

    pub(crate) fn render<B: Backend>(
        self,
        f: &mut Frame<B>,
        rect: Rect,
        history: &CommandsHistory,
    ) {
        let history_index = self
            .state
            .history_index
            .and_then(|index| history.get(index));
        match history_index {
            Some(text) => ReadonlyEntry::new(text)
                .prefix(self.prefix)
                .style(self.style)
                .render(f, rect),
            None => Entry::new().prefix(self.prefix).style(self.style).render(
                f,
                rect,
                &mut self.state.buffer,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use tui::backend::TestBackend;
    use tui::buffer::Buffer;
    use tui::layout::Rect;
    use tui::style::Style;
    use tui::text::Span;
    use tui::Terminal;

    use super::{CommandActionResult, CommandEditor, CommandState};
    use crate::events::key;
    use crate::history::CommandsHistory;

    fn assert_display(state: &mut CommandState, history: &CommandsHistory, display: &str) {
        let mut terminal = Terminal::new(TestBackend::new(80, 1)).unwrap();
        terminal
            .draw(|f| {
                CommandEditor::new(state).prefix(Span::raw(":")).render(
                    f,
                    Rect::new(0, 0, 80, 1),
                    history,
                );
            })
            .unwrap();

        let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 1));
        buffer.set_string(0, 0, display, Style::default());
        terminal.backend().assert_buffer(&buffer);
    }

    #[test]
    fn navigating_history() {
        let mut history = CommandsHistory::new();
        history.push("first").unwrap();
        history.push("second").unwrap();

        let mut state = CommandState::default();
        assert_display(&mut state, &history, ":");

        assert_eq!(
            state.handle_event(key!(Up), &history),
            CommandActionResult::Update
        );
        assert_display(&mut state, &history, ":second");

        assert_eq!(
            state.handle_event(key!(Up), &history),
            CommandActionResult::Update
        );
        assert_display(&mut state, &history, ":first");

        assert_eq!(
            state.handle_event(key!(Up), &history),
            CommandActionResult::None
        );
        assert_display(&mut state, &history, ":first");

        assert_eq!(
            state.handle_event(key!(Down), &history),
            CommandActionResult::Update
        );
        assert_display(&mut state, &history, ":second");

        assert_eq!(
            state.handle_event(key!(Down), &history),
            CommandActionResult::Update
        );
        assert_display(&mut state, &history, ":");
    }

    #[test]
    fn keeps_the_entered_command() {
        let mut history = CommandsHistory::new();
        history.push("first").unwrap();
        history.push("second").unwrap();
        let mut state = CommandState::default();

        assert_eq!(
            state.handle_event(key!('f'), &history),
            CommandActionResult::Update
        );
        assert_display(&mut state, &history, ":f");

        assert_eq!(
            state.handle_event(key!(Up), &history),
            CommandActionResult::Update
        );
        assert_display(&mut state, &history, ":first");

        assert_eq!(
            state.handle_event(key!(Down), &history),
            CommandActionResult::Update
        );
        assert_display(&mut state, &history, ":f");

        assert_eq!(
            state.handle_event(key!(Enter), &history),
            CommandActionResult::Submit
        );
        assert_eq!(state.as_str(&history), "f");
    }

    #[test]
    fn submits_history_command() {
        let mut history = CommandsHistory::new();
        history.push("first").unwrap();
        let mut state = CommandState::default();
        state.handle_event(key!(Up), &history);

        assert_eq!(
            state.handle_event(key!(Enter), &history),
            CommandActionResult::Submit
        );
        assert_eq!(state.as_str(&history), "first");
    }

    #[test]
    fn updates_from_history() {
        let mut history = CommandsHistory::new();
        history.push("first").unwrap();
        let mut state = CommandState::default();
        state.handle_event(key!(Up), &history);

        assert_eq!(
            state.handle_event(key!(Backspace), &history),
            CommandActionResult::Update
        );
        assert_eq!(state.as_str(&history), "firs");
    }

    #[test]
    fn clears_via_backspace() {
        let mut state = CommandState::default();
        let history = CommandsHistory::new();
        state.handle_event(key!('a'), &history);
        assert_eq!(
            state.handle_event(key!(Backspace), &history),
            CommandActionResult::Update
        );
        assert_eq!(
            state.handle_event(key!(Backspace), &history),
            CommandActionResult::Clear
        );
    }

    #[test]
    fn clears_via_clear() {
        let mut state = CommandState::default();
        let history = CommandsHistory::new();
        assert_eq!(
            state.handle_event(key!(Enter), &history),
            CommandActionResult::Clear
        );
    }
}
