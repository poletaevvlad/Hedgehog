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
use cluFlock::ToFlock;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use directories::BaseDirs;
use hedgehog_library::datasource::DataProvider;
use hedgehog_library::status_writer::StatusWriter;
use hedgehog_library::{opml, Library, SqliteDataProvider};
use hedgehog_player::Player;
use screen::UI;
use std::fmt;
use std::fs::OpenOptions;
use std::io::{self, BufReader, SeekFrom};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use tui::backend::CrosstermBackend;
use tui::Terminal;

#[derive(Debug, thiserror::Error)]
#[error("Data directory cannot be determined")]
struct CannotDetermineDataDirectory;

#[derive(Debug, thiserror::Error)]
struct AlreadyRunningError {
    pid: Option<String>,
}

impl fmt::Display for AlreadyRunningError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Another instance of Hedgehog is already running")?;
        if let Some(ref pid) = self.pid {
            f.write_fmt(format_args!(" (PID {})", pid))?;
        }
        Ok(())
    }
}

pub(crate) struct AppContext {
    data_path: PathBuf,
    config_path: Vec<PathBuf>,
}

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
        .arg(
            clap::Arg::with_name("no_mouse")
                .long("no-mouse")
                .help("Disables mouse support"),
        )
        .arg(
            clap::Arg::with_name("data_path")
                .long("data-path")
                .takes_value(true)
                .value_name("DIR")
                .help("Location for the episodes database and other data"),
        )
        .arg(
            clap::Arg::with_name("config_path")
                .long("config-path")
                .takes_value(true)
                .value_name("DIRS")
                .env("HEDGEHOG_PATH")
                .help("Locations for the theme and rc files"),
        )
        .get_matches();

    let result = (|| {
        let base_dirs = BaseDirs::new().ok_or(CannotDetermineDataDirectory)?;
        let mut data_dir = match cli_args.value_of("data_path") {
            Some(data_path) => Path::new(data_path).to_owned(),
            None => {
                let mut data_dir = base_dirs.data_dir().to_path_buf();
                data_dir.push("hedgehog");
                data_dir
            }
        };
        std::fs::create_dir_all(&data_dir)?;

        data_dir.push("pid");
        let mut pidfile = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&data_dir)?;
        let mut previous_pid = String::new();
        pidfile.read_to_string(&mut previous_pid)?;

        let pid_lock = match pidfile.try_exclusive_lock() {
            Ok(mut lock) => {
                lock.seek(SeekFrom::Start(0))?;
                writeln!(lock, "{}", std::process::id())?;
                let position = lock.seek(SeekFrom::Current(0))?;
                lock.set_len(position)?;
                lock
            }
            Err(_) => {
                previous_pid.truncate(previous_pid.trim_end().len());
                return Err(AlreadyRunningError {
                    pid: if previous_pid.is_empty() {
                        None
                    } else {
                        Some(previous_pid)
                    },
                }
                .into());
            }
        };

        data_dir.pop();

        data_dir.push("episodes");
        let data_provider = SqliteDataProvider::connect(&data_dir)?;
        data_dir.pop();

        let mut config_path = Vec::new();
        if cfg!(unix) {
            config_path.push(Path::new("/usr/share/hedgehog").to_path_buf());
        } else if cfg!(windows) {
            if let Ok(mut exe_path) = std::env::current_exe() {
                exe_path.pop();
                exe_path.push("config");
                config_path.push(exe_path);
            }
        }

        let mut user_config_dir = base_dirs.config_dir().to_path_buf();
        user_config_dir.push("hedgehog");
        config_path.push(user_config_dir);

        if let Some(paths) = cli_args.value_of("config_path") {
            config_path.extend(std::env::split_paths(paths));
        }

        let context = AppContext {
            data_path: data_dir,
            config_path,
        };

        let result = match cli_args.subcommand() {
            ("export", Some(args)) => run_export(&data_provider, args),
            ("import", Some(args)) => run_import(&data_provider, args),
            _ => run_player(data_provider, &cli_args, context),
        };
        drop(pid_lock);
        result
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
    args: &ArgMatches,
    ctx: AppContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        System::current().stop_with_code(1);
        default_hook(info);
    }));

    let enable_mouse = !args.is_present("no_mouse");

    let system = System::new();
    Player::initialize()?;

    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    if enable_mouse {
        execute!(io::stdout(), EnableMouseCapture)?;
    }

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

        run_mpris(player.clone(), player_arbiter.handle());

        UI::new(
            (size.width, size.height),
            terminal,
            library,
            player,
            status_writer,
            ctx,
        )
        .start();
    });
    system.run()?;

    execute!(io::stdout(), LeaveAlternateScreen)?;
    if enable_mouse {
        execute!(io::stdout(), DisableMouseCapture)?;
    }
    disable_raw_mode()?;
    Ok(())
}

#[cfg(feature = "mpris")]
fn run_mpris(player: Addr<Player>, arbiter: ArbiterHandle) {
    use hedgehog_player::mpris::MprisPlayer;
    MprisPlayer::start_in_arbiter(&arbiter, |_| MprisPlayer::new(player));
}

#[cfg(not(feature = "mpris"))]
fn run_mpris(_player: Addr<Player>, _arbiter: ArbiterHandle) {}
