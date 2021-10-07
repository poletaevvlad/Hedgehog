mod events;
use actix::prelude::*;
use crossterm::event::{Event, KeyEvent};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use events::key;
use std::io;
use tui::backend::CrosstermBackend;
use tui::Terminal;

struct UI {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    counter: u64,
}

impl UI {
    fn new(terminal: Terminal<CrosstermBackend<std::io::Stdout>>) -> Self {
        UI {
            terminal,
            counter: 0,
        }
    }

    fn render(&mut self) {
        let counter = self.counter;
        self.terminal
            .draw(|f| {
                let size = f.size();
                let block = tui::widgets::Block::default()
                    .title(format!("{}", counter))
                    .borders(tui::widgets::Borders::ALL);
                f.render_widget(block, size);
            })
            .map_err(|err| {
                println!("{:?}", err);
                err
            })
            .unwrap();
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
    let terminal = Terminal::new(backend)?;
    system.block_on(async {
        UI::new(terminal).start();
    });
    system.run()?;

    execute!(io::stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
