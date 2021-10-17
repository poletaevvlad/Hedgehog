mod cmdparser;
mod dataview;
mod events;
mod history;
mod paging;
mod screen;
mod status;
mod view_model;
mod widgets;

use actix::prelude::*;
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use hedgehog_library::{Library, SqliteDataProvider};
use screen::UI;
use std::io;
use tui::backend::CrosstermBackend;
use tui::Terminal;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let system = System::new();
    let data_provider = SqliteDataProvider::connect_default_path()?;

    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    system.block_on(async {
        let library = Library::new(data_provider).start();
        UI::new(terminal, library).start();
    });
    system.run()?;

    execute!(io::stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
