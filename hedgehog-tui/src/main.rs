mod events;
mod widgets;
use crate::widgets::textentry;
use actix::prelude::*;
use crossterm::event::{Event, KeyEvent};
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

struct UI {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    command: textentry::Buffer,
}

impl UI {
    fn new(terminal: Terminal<CrosstermBackend<std::io::Stdout>>) -> Self {
        UI {
            terminal,
            command: textentry::Buffer::default(),
        }
    }

    fn render(&mut self) {
        let command = &mut self.command;
        let draw = |f: &mut tui::Frame<CrosstermBackend<std::io::Stdout>>| {
            let size = f.size();
            f.render_widget(
                tui::widgets::Block::default().borders(tui::widgets::Borders::ALL),
                Rect::new(19, 9, 8, 3),
            );
            textentry::Entry::new().prefix(Span::raw("::")).render(
                f,
                Rect::new(20, 10, 6, 1),
                command,
            );
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
        let should_render = match item {
            Ok(key!('c', CONTROL)) => {
                System::current().stop();
                false
            }
            Ok(Event::Resize(_, _)) => true,
            Ok(event) => self.command.handle_event(event),
            Err(_) => {
                System::current().stop();
                false
            }
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
