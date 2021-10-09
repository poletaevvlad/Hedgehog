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
                Rect::new(9, 9, size.width - 18, 3),
            );
            textentry::Entry::new().prefix(Span::raw(":")).render(
                f,
                Rect::new(10, 10, size.width - 20, 1),
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
            key!('c', CONTROL) => {
                System::current().stop();
                false
            }
            Ok(crossterm::event::Event::Key(crossterm::event::KeyEvent {
                code: crossterm::event::KeyCode::Char(ch),
                modifiers: crossterm::event::KeyModifiers::NONE,
            })) => {
                self.command.push_char(ch);
                true
            }
            key!(Left) => self
                .command
                .move_cursor(textentry::Direction::Backward, textentry::Amount::Character),
            key!(Right) => self
                .command
                .move_cursor(textentry::Direction::Forward, textentry::Amount::Character),
            key!(Left, CONTROL) => self
                .command
                .move_cursor(textentry::Direction::Backward, textentry::Amount::Word),
            key!(Right, CONTROL) => self
                .command
                .move_cursor(textentry::Direction::Forward, textentry::Amount::Word),
            key!(Home) => self
                .command
                .move_cursor(textentry::Direction::Backward, textentry::Amount::All),
            key!(End) => self
                .command
                .move_cursor(textentry::Direction::Forward, textentry::Amount::All),
            key!(Backspace) => self
                .command
                .delete(textentry::Direction::Backward, textentry::Amount::Character),
            key!(Delete) => self
                .command
                .delete(textentry::Direction::Forward, textentry::Amount::Character),
            key!(Backspace, SHIFT) => self
                .command
                .delete(textentry::Direction::Backward, textentry::Amount::All),
            key!(Delete, SHIFT) => self
                .command
                .delete(textentry::Direction::Forward, textentry::Amount::All),
            Ok(Event::Resize(_, _)) => true,
            Err(_) => {
                System::current().stop();
                false
            }
            _ => false,
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
