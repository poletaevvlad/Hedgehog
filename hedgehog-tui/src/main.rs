mod cmdparser;
mod cmdreader;
mod dataview;
mod events;
mod history;
mod keymap;
mod options;
mod screen;
mod status;
mod theming;
mod view_model;
mod widgets;

use actix::prelude::*;
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use hedgehog_library::{Library, SqliteDataProvider};
use hedgehog_player::Player;
use screen::UI;
use std::io;
use tui::backend::CrosstermBackend;
use tui::Terminal;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let system = System::new();
    let data_provider = SqliteDataProvider::connect_default_path()?;

    Player::initialize()?;

    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let size = terminal.size()?;
    terminal.clear()?;

    system.block_on(async {
        let library_arbiter = Arbiter::new();
        let library =
            Library::start_in_arbiter(&library_arbiter.handle(), |_| Library::new(data_provider));
        let player_arbiter = Arbiter::new();
        let player = Player::start_in_arbiter(
            &player_arbiter.handle(),
            |_| /* TODO */ Player::init().unwrap(),
        );
        let ui = UI::new((size.width, size.height), terminal, library.clone(), player).start();
        library.do_send(hedgehog_library::FeedUpdateRequest::Subscribe(
            ui.recipient(),
        ))
    });
    system.run()?;

    execute!(io::stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
