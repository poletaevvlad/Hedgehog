use crate::cmdparser;
use crate::events::key;
use crate::history::CommandsHistory;
use crate::status::{Severity, Status};
use crate::widgets::command::{CommandActionResult, CommandEditor, CommandState};
use actix::prelude::*;
use crossterm::event::Event;
use tui::backend::CrosstermBackend;
use tui::layout::Rect;
use tui::style::{Color, Style};
use tui::text::Span;
use tui::widgets::Paragraph;
use tui::Terminal;

pub(crate) struct UI {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    command: Option<CommandState>,
    commands_history: CommandsHistory,
    status: Option<Status>,
}

impl UI {
    pub(crate) fn new(terminal: Terminal<CrosstermBackend<std::io::Stdout>>) -> Self {
        UI {
            terminal,
            command: None,
            commands_history: CommandsHistory::new(),
            status: None,
        }
    }

    fn render(&mut self) {
        let command = &mut self.command;
        let history = &self.commands_history;
        let status = &self.status;

        let draw = |f: &mut tui::Frame<CrosstermBackend<std::io::Stdout>>| {
            let area = f.size();
            f.render_widget(
                tui::widgets::Paragraph::new(tui::text::Spans(vec![Span::raw(format!(
                    "{:?}, {:?}",
                    command, history
                ))])),
                Rect::new(0, 0, area.width, area.height - 1),
            );

            let status_rect = Rect::new(0, area.height - 1, area.width, 1);
            if let Some(ref mut command_state) = command {
                CommandEditor::new(command_state)
                    .prefix(Span::raw(":"))
                    .render(f, status_rect, history);
            } else if let Some(status) = status {
                let color = match status.severity() {
                    Severity::Error => Color::Red,
                    Severity::Warning => Color::Yellow,
                    Severity::Information => Color::LightBlue,
                };
                f.render_widget(
                    Paragraph::new(status.to_string()).style(Style::default().fg(color)),
                    status_rect,
                );
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
            None => match event {
                key!('c', CONTROL) => {
                    System::current().stop();
                    false
                }
                key!(':') => {
                    self.status = None;
                    self.command = Some(CommandState::default());
                    true
                }
                _ => false,
            },
            Some(ref mut command_state) => {
                match command_state.handle_event(event, &self.commands_history) {
                    CommandActionResult::None => false,
                    CommandActionResult::Update => true,
                    CommandActionResult::Clear => {
                        self.command = None;
                        true
                    }
                    CommandActionResult::Submit => {
                        let command_str = command_state.as_str(&self.commands_history).to_string();
                        match cmdparser::from_str::<()>(&command_str) {
                            Ok(_cmd) => (),
                            Err(error) => self.status = Some(error.into()),
                        };
                        self.commands_history.push(&command_str);
                        self.command = None;
                        true
                    }
                }
            }
        };
        if should_render {
            self.render();
        }
    }
}
