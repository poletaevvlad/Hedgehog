mod events;
mod widgets;
use crate::widgets::textentry;
use actix::prelude::*;
use crossterm::event::Event;
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use events::key;
use std::io;
use tui::backend::CrosstermBackend;
use tui::layout::Rect;
use tui::text::Span;
use tui::Terminal;

#[derive(Debug)]
enum CommandState {
    None,
    Command(textentry::Buffer),
}

struct UI {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    command: CommandState,
}

impl UI {
    fn new(terminal: Terminal<CrosstermBackend<std::io::Stdout>>) -> Self {
        UI {
            terminal,
            command: CommandState::None,
        }
    }

    fn render(&mut self) {
        let command = &mut self.command;
        let draw = |f: &mut tui::Frame<CrosstermBackend<std::io::Stdout>>| {
            let area = f.size();
            f.render_widget(
                tui::widgets::Paragraph::new(tui::text::Spans(vec![Span::raw(format!(
                    "{:?}",
                    command
                ))])),
                Rect::new(0, 0, area.width, area.height - 1),
            );

            match command {
                CommandState::None => (),
                CommandState::Command(ref mut buffer) => {
                    let entry = textentry::Entry::new().prefix(Span::raw(":"));
                    entry.render(f, Rect::new(0, area.height - 1, area.width, 1), buffer);
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
                    self.command = CommandState::Command(textentry::Buffer::default());
                    true
                }
                _ => false,
            },
            CommandState::Command(ref mut buffer) => match event {
                key!('c', CONTROL) | key!(Esc) => {
                    self.command = CommandState::None;
                    true
                }
                key!(Backspace) if buffer.is_empty() => {
                    self.command = CommandState::None;
                    true
                }
                event => buffer.handle_event(event),
            },
        };
        if should_render {
            self.render();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let system = System::new();

    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    system.block_on(async {
        UI::new(terminal).start();
    });
    system.run()?;

    execute!(io::stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
