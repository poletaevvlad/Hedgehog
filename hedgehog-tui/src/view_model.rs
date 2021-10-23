use super::screen::EpisodesListProvider;
use crate::cmdparser;
use crate::dataview::{CursorCommand, InteractiveList, PaginatedData};
use crate::keymap::{Key, KeyMapping};
use crate::status::{Severity, Status};
use crate::theming::{Theme, ThemeCommand};
use actix::System;
use hedgehog_library::model::EpisodeSummary;
use serde::Deserialize;

pub(crate) struct ViewModel {
    pub(crate) episodes_list: InteractiveList<PaginatedData<EpisodeSummary>, EpisodesListProvider>,
    pub(crate) status: Option<Status>,
    pub(crate) key_mapping: KeyMapping<Command>,
    pub(crate) theme: Theme,
}

impl ViewModel {
    pub(crate) fn new(size: (u16, u16)) -> Self {
        ViewModel {
            episodes_list: InteractiveList::new(size.1 as usize - 1),
            status: None,
            key_mapping: KeyMapping::new(),
            theme: Theme::default(),
        }
    }

    pub(crate) fn set_size(&mut self, _width: u16, height: u16) {
        self.episodes_list.set_window_size(height as usize - 1)
    }

    pub(crate) fn clear_status(&mut self) {
        self.status = None;
    }

    pub(crate) fn handle_command_str(&mut self, command: &str) {
        match cmdparser::from_str(command) {
            Ok(command) => {
                self.handle_command_interactive(command);
            }
            Err(error) => self.status = Some(Status::CommandParsingError(error)),
        }
    }

    fn handle_command(&mut self, command: Command) -> Result<bool, Status> {
        match command {
            Command::Cursor(command) => {
                self.episodes_list.handle_command(command);
                Ok(true)
            }
            Command::Quit => {
                System::current().stop();
                Ok(false)
            }
            Command::Map(key, command) => {
                let redefined = self.key_mapping.contains(&key);
                self.key_mapping.map(key, *command);

                if redefined {
                    Err(Status::new_custom(
                        "Key mapping redefined",
                        Severity::Information,
                    ))
                } else {
                    Ok(false)
                }
            }
            Command::Unmap(key) => {
                if !self.key_mapping.unmap(&key) {
                    Err(Status::new_custom(
                        "Key mapping is not defined",
                        Severity::Warning,
                    ))
                } else {
                    Ok(false)
                }
            }
            Command::Theme(command) => self
                .theme
                .handle_command(command)
                .map(|_| true)
                .map_err(|error| Status::new_custom(format!("{}", error), Severity::Error)),
        }
    }

    pub(crate) fn handle_command_interactive(&mut self, command: Command) -> bool {
        match self.handle_command(command) {
            Ok(should_redraw) => should_redraw,
            Err(status) => {
                self.status = Some(status);
                true
            }
        }
    }
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Command {
    #[serde(rename = "line")]
    Cursor(CursorCommand),
    Map(Key, Box<Command>),
    Unmap(Key),
    Theme(ThemeCommand),
    #[serde(alias = "q")]
    Quit,
}
