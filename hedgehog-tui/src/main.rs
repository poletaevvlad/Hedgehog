mod cmdreader;
mod events;
mod history;
mod keymap;
mod options;
mod screen;
mod scrolling;
mod status;
mod theming;
mod widgets;

use actix::prelude::*;
use crossterm::event::EnableMouseCapture;
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use hedgehog_library::status_writer::StatusWriter;
use hedgehog_library::{Library, SqliteDataProvider};
use hedgehog_player::mpris::MprisPlayer;
use hedgehog_player::Player;
use screen::UI;
use std::io;
use tui::backend::CrosstermBackend;
use tui::Terminal;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        System::current().stop_with_code(1);
        default_hook(info);
    }));

    let system = System::new();
    let data_provider = SqliteDataProvider::connect_default_path()?;

    Player::initialize()?;

    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let size = terminal.size()?;
    terminal.clear()?;

    system.block_on(async {
        let library_arbiter = Arbiter::new();
        let library =
            Library::start_in_arbiter(&library_arbiter.handle(), |_| Library::new(data_provider));
        let status_writer = StatusWriter::new(library.clone()).start();

        let player_arbiter = Arbiter::new();
        let player = Player::start_in_arbiter(
            &player_arbiter.handle(),
            |_| /* TODO */ Player::init().unwrap(),
        );

        let mpirs_player = player.clone();
        MprisPlayer::start_in_arbiter(&player_arbiter.handle(), |_| MprisPlayer::new(mpirs_player));

        UI::new(
            (size.width, size.height),
            terminal,
            library,
            player,
            status_writer,
        )
        .start();
    });
    system.run()?;

    execute!(io::stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
