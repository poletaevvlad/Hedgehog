use super::textentry::{Buffer, Entry, ReadonlyEntry};
use crate::events::key;
use crate::history::CommandsHistory;
use crossterm::event::Event;
use tui::backend::Backend;
use tui::layout::Rect;
use tui::text::Span;
use tui::Frame;

#[derive(Debug)]
pub(crate) struct CommandState {
    buffer: Buffer,
    history_index: Option<usize>,
}

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
                        let history_index = history_index
                            .map(|index| index.saturating_add(1))
                            .unwrap_or(0);
                        history.find_before(history_index, self.buffer.as_str())
                    }
                    key!(Down) => {
                        let history_index = history_index
                            .map(|index| index.saturating_sub(1))
                            .unwrap_or(0);
                        history.find_after(history_index, self.buffer.as_str())
                    }
                    _ => unreachable!(),
                };

                match found_index {
                    None => CommandActionResult::None,
                    Some(new_index) => {
                        self.history_index = Some(new_index);
                        CommandActionResult::Update
                    }
                }
            }
            key!(Enter) if self.buffer.is_empty() => CommandActionResult::Clear,
            key!(Enter) => CommandActionResult::Submit,
            event if Buffer::is_editing_event(event) => {
                let history_str = self.history_index.and_then(|index| history.get(index));
                if let Some(string) = history_str {
                    self.buffer = Buffer::from(string.to_string());
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
}

impl<'a> CommandEditor<'a> {
    pub(crate) fn new(state: &'a mut CommandState) -> Self {
        CommandEditor {
            state,
            prefix: None,
        }
    }

    pub(crate) fn prefix(mut self, prefix: Span<'a>) -> Self {
        self.prefix = Some(prefix);
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
            Some(text) => ReadonlyEntry::new(text).prefix(self.prefix).render(f, rect),
            None => Entry::new()
                .prefix(self.prefix)
                .render(f, rect, &mut self.state.buffer),
        }
    }
}
