use crate::events::key;
use crate::history::CommandsHistory;
use crate::widgets::textentry;
use actix::prelude::*;
use crossterm::event::Event;
use tui::backend::CrosstermBackend;
use tui::layout::Rect;
use tui::text::Span;
use tui::Terminal;

#[derive(Debug)]
enum CommandState {
    None,
    Command(textentry::Buffer, Option<usize>),
}

pub(crate) struct UI {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    command: CommandState,
    commands_history: CommandsHistory,
}

impl UI {
    pub(crate) fn new(terminal: Terminal<CrosstermBackend<std::io::Stdout>>) -> Self {
        UI {
            terminal,
            command: CommandState::None,
            commands_history: CommandsHistory::new(),
        }
    }

    fn render(&mut self) {
        let command = &mut self.command;
        let history = &self.commands_history;
        let draw = |f: &mut tui::Frame<CrosstermBackend<std::io::Stdout>>| {
            let area = f.size();
            f.render_widget(
                tui::widgets::Paragraph::new(tui::text::Spans(vec![Span::raw(format!(
                    "{:?}, {:?}",
                    command, history
                ))])),
                Rect::new(0, 0, area.width, area.height - 1),
            );

            match command {
                CommandState::None => (),
                CommandState::Command(ref mut buffer, history_index) => {
                    let history_index = history_index.and_then(|index| history.get(index));
                    let rect = Rect::new(0, area.height - 1, area.width, 1);
                    match history_index {
                        Some(text) => textentry::ReadonlyEntry::new(text)
                            .prefix(Span::raw(":"))
                            .render(f, rect),
                        None => {
                            let entry = textentry::Entry::new().prefix(Span::raw(":"));
                            entry.render(f, rect, buffer);
                        }
                    }
                }
            }
        };
        self.terminal.draw(draw).unwrap();
    }
}

impl Actor for UI {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.add_stream(crossterm::event::EventStream::new());
        self.render();
    }
}

impl StreamHandler<crossterm::Result<crossterm::event::Event>> for UI {
    fn handle(
        &mut self,
        item: crossterm::Result<crossterm::event::Event>,
        _ctx: &mut Self::Context,
    ) {
        let event = match item {
            Ok(Event::Resize(_, _)) => {
                self.render();
                return;
            }
            Ok(event) => event,
            Err(_) => {
                System::current().stop();
                return;
            }
        };

        let should_render = match self.command {
            CommandState::None => match event {
                key!('c', CONTROL) => {
                    System::current().stop();
                    false
                }
                key!(':') => {
                    self.command = CommandState::Command(textentry::Buffer::default(), None);
                    true
                }
                _ => false,
            },
            CommandState::Command(ref mut buffer, history_index) => match event {
                key!('c', CONTROL) | key!(Esc) => {
                    self.command = CommandState::None;
                    true
                }
                key!(Backspace) if buffer.is_empty() && history_index.is_none() => {
                    self.command = CommandState::None;
                    true
                }
                key!(Up) | key!(Down) => {
                    let found_index = match event {
                        key!(Up) => {
                            let history_index = history_index.map(|index| index + 1).unwrap_or(0);
                            self.commands_history
                                .find_before(history_index, buffer.as_str())
                        }
                        key!(Down) => {
                            let history_index = history_index
                                .map(|index| index.saturating_sub(1))
                                .unwrap_or(0);
                            self.commands_history
                                .find_after(history_index, buffer.as_str())
                        }
                        _ => unreachable!(),
                    };
                    match found_index {
                        None => false,
                        Some(new_index) => {
                            let current_state =
                                std::mem::replace(&mut self.command, CommandState::None);
                            if let CommandState::Command(buffer, _) = current_state {
                                self.command = CommandState::Command(buffer, Some(new_index));
                                true
                            } else {
                                unreachable!()
                            }
                        }
                    }
                }
                key!(Enter) => {
                    let command = buffer.as_str();
                    if !command.is_empty() {
                        self.commands_history.push(command.to_string());
                    }
                    self.command = CommandState::None;
                    true
                }
                event if textentry::Buffer::is_editing_event(event) => {
                    let history = &self.commands_history;
                    let history_str = history_index.and_then(|index| history.get(index));
                    match history_str {
                        Some(string) => {
                            let mut new_buffer = textentry::Buffer::from(string.to_string());
                            new_buffer.handle_event(event);
                            self.command = CommandState::Command(new_buffer, None);
                            true
                        }
                        None => buffer.handle_event(event),
                    }
                }
                _ => false,
            },
        };
        if should_render {
            self.render();
        }
    }
}
