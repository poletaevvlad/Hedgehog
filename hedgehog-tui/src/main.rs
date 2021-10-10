mod cmdparser;
mod events;
mod history;
mod screen;
mod widgets;
use actix::prelude::*;
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use screen::UI;
use std::io;
use tui::backend::CrosstermBackend;
use tui::Terminal;

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
