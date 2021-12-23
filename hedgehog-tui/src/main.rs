mod cmdreader;
mod events;
mod history;
mod keymap;
mod mouse;
mod options;
mod screen;
mod scrolling;
mod status;
mod theming;
mod widgets;

use actix::prelude::*;
use clap::ArgMatches;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use hedgehog_library::datasource::DataProvider;
use hedgehog_library::status_writer::StatusWriter;
use hedgehog_library::{opml, Library, SqliteDataProvider};
use hedgehog_player::mpris::MprisPlayer;
use hedgehog_player::Player;
use screen::UI;
use std::fs::OpenOptions;
use std::io::{self, BufReader};
use tui::backend::CrosstermBackend;
use tui::Terminal;

fn main() {
    let cli_args = clap::App::new("Hedgehog")
        .version(clap::crate_version!())
        .about(clap::crate_description!())
        .subcommand(
            clap::SubCommand::with_name("export")
                .about("Export the list of podcast as an OPML file")
                .arg(
                    clap::Arg::with_name("output")
                        .long("output")
                        .short("o")
                        .value_name("FILE")
                        .takes_value(true)
                        .help("A file path where the OPML file will be written"),
                ),
        )
        .subcommand(
            clap::SubCommand::with_name("import")
                .about("Import podcasts from the OPML file")
                .arg(
                    clap::Arg::with_name("file")
                        .required(true)
                        .value_name("FILE")
                        .help("A path to the OPML file or '-' for standard input"),
                ),
        )
        .get_matches();

    let result = (|| {
        let data_provider = SqliteDataProvider::connect_default_path()?;
        match cli_args.subcommand() {
            ("export", Some(args)) => run_export(&data_provider, args),
            ("import", Some(args)) => run_import(&data_provider, args),
            _ => run_player(data_provider, &cli_args),
        }
    })();

    if let Err(error) = result {
        eprintln!("Erorr: {}", error);
        std::process::exit(1);
    }
}

fn run_export<P: DataProvider>(
    data_provider: &P,
    args: &ArgMatches,
) -> Result<(), Box<dyn std::error::Error>> {
    let output = args.value_of("output");
    match output {
        Some(path) => {
            let file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(path)?;
            opml::build_opml(file, data_provider)?;
        }
        None => opml::build_opml(io::stdout(), data_provider)?,
    }
    Ok(())
}

fn run_import<P: DataProvider>(
    data_provider: &P,
    args: &ArgMatches,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = args.value_of("file").expect("arg is required");
    match file {
        "-" => opml::import_opml(std::io::stdin().lock(), data_provider)?,
        file => opml::import_opml(
            BufReader::new(OpenOptions::new().read(true).open(file)?),
            data_provider,
        )?,
    }
    Ok(())
}

fn run_player(
    data_provider: SqliteDataProvider,
    _args: &ArgMatches,
) -> Result<(), Box<dyn std::error::Error>> {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        System::current().stop_with_code(1);
        default_hook(info);
    }));

    let system = System::new();

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

    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    disable_raw_mode()?;
    Ok(())
}
