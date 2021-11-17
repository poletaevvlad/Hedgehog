use crate::cmdreader::{CommandReader, FileResolver};
use crate::dataview::{
    CursorCommand, InteractiveList, ListData, PaginatedData, PaginatedDataMessage, Versioned,
};
use crate::keymap::{Key, KeyMapping};
use crate::options::{Options, OptionsUpdate};
use crate::screen::{EpisodesListProvider, FeedsListProvider};
use crate::status::{Severity, Status};
use crate::theming::{Theme, ThemeCommand};
use actix::System;
use cmd_parser::CmdParsable;
use hedgehog_library::model::{
    EpisodeId, EpisodeSummary, EpisodeSummaryStatus, FeedId, FeedSummary,
};
use hedgehog_library::{
    EpisodesQuery, FeedUpdateNotification, FeedUpdateRequest, FeedUpdateResult,
};
use hedgehog_player::state::PlaybackState;
use hedgehog_player::{volume::VolumeCommand, PlaybackCommand, PlayerNotification};
use std::collections::HashSet;
use std::path::PathBuf;

pub(crate) trait ActionDelegate {
    fn start_playback(&self, episode_id: EpisodeId);
    fn send_volume_command(&self, command: VolumeCommand);
    fn send_playback_command(&self, command: PlaybackCommand);
    fn send_feed_update_request(&self, command: FeedUpdateRequest);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, CmdParsable)]
pub(crate) enum FocusedPane {
    #[cmd(rename = "feeds")]
    FeedsList,
    #[cmd(rename = "episodes")]
    EpisodesList,
}

pub(crate) struct ViewModel<D> {
    pub(crate) options: Options,
    pub(crate) feeds_list: InteractiveList<ListData<FeedSummary>, FeedsListProvider>,
    pub(crate) episodes_list: InteractiveList<PaginatedData<EpisodeSummary>, EpisodesListProvider>,
    pub(crate) status: Option<Status>,
    pub(crate) key_mapping: KeyMapping<Command, FocusedPane>,
    pub(crate) theme: Theme,
    pub(crate) focus: FocusedPane,
    selected_feed: Option<FeedId>,
    pub(crate) playing_episode: Option<EpisodeSummary>,
    pub(crate) playback_state: PlaybackState,
    pub(crate) action_delegate: D,
    pub(crate) updating_feeds: HashSet<FeedId>,
}

impl<D: ActionDelegate> ViewModel<D> {
    pub(crate) fn new(size: (u16, u16), action_delegate: D) -> Self {
        ViewModel {
            options: Options::default(),
            feeds_list: InteractiveList::new(size.1 as usize - 2),
            episodes_list: InteractiveList::new(size.1 as usize - 2),
            status: None,
            key_mapping: KeyMapping::new(),
            theme: Theme::default(),
            focus: FocusedPane::FeedsList,
            selected_feed: None,
            playing_episode: None,
            playback_state: PlaybackState::default(),
            action_delegate,
            updating_feeds: HashSet::new(),
        }
    }

    pub(crate) fn set_size(&mut self, _width: u16, height: u16) {
        self.episodes_list.set_window_size(height as usize - 2);
        self.feeds_list.set_window_size(height as usize - 2);
    }

    pub(crate) fn clear_status(&mut self) {
        self.status = None;
    }

    pub(crate) fn error(&mut self, error: impl std::error::Error) {
        self.status = Some(Status::new_custom(error.to_string(), Severity::Error));
    }

    pub(crate) fn handle_command_str(&mut self, command: &str) {
        match Command::parse_cmd_full(command) {
            Ok(command) => {
                self.handle_command_interactive(command);
            }
            Err(error) => self.status = Some(Status::CommandParsingError(error.into_static())),
        }
    }

    fn handle_command(&mut self, command: Command) -> Result<bool, Status> {
        match command {
            Command::Cursor(command) => {
                match self.focus {
                    FocusedPane::FeedsList => {
                        self.feeds_list.handle_command(command);
                        self.update_current_feed();
                    }
                    FocusedPane::EpisodesList => self.episodes_list.handle_command(command),
                }
                Ok(true)
            }
            Command::SetFocus(focused_pane) => {
                if self.focus != focused_pane {
                    self.focus = focused_pane;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Command::Quit => {
                System::current().stop();
                Ok(false)
            }
            Command::Map(key, command) => {
                let redefined = self.key_mapping.contains(key, None);
                self.key_mapping.map(key, None, *command);

                if redefined {
                    Err(Status::new_custom(
                        "Key mapping redefined",
                        Severity::Information,
                    ))
                } else {
                    Ok(false)
                }
            }
            Command::MapState(key, state, command) => {
                let redefined = self.key_mapping.contains(key, None);
                self.key_mapping.map(key, Some(state), *command);

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
                if !self.key_mapping.unmap(key, None) {
                    Err(Status::new_custom(
                        "Key mapping is not defined",
                        Severity::Warning,
                    ))
                } else {
                    Ok(false)
                }
            }
            Command::UnmapState(key, state) => {
                if !self.key_mapping.unmap(key, Some(state)) {
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
            Command::Exec(path) => {
                let mut reader = match CommandReader::open(path) {
                    Ok(reader) => reader,
                    Err(error) => {
                        return Err(Status::new_custom(format!("{}", error), Severity::Error))
                    }
                };

                loop {
                    match reader.read() {
                        Ok(None) => break Ok(true),
                        Ok(Some(command)) => {
                            if let Err(status) = self.handle_command(command) {
                                if status.severity() == Severity::Error {
                                    return Err(status);
                                }
                            }
                        }
                        Err(error) => {
                            return Err(Status::new_custom(format!("{}", error), Severity::Error))
                        }
                    }
                }
            }
            Command::Volume(command) => {
                self.action_delegate.send_volume_command(command);
                Ok(false)
            }
            Command::PlayCurrent => {
                if let Some(current_episode) = self.episodes_list.selection() {
                    if Some(current_episode.id)
                        == self.playing_episode.as_ref().map(|episode| episode.id)
                    {
                        return Ok(false);
                    }
                    self.action_delegate.start_playback(current_episode.id);
                    self.playing_episode = Some(current_episode.clone());
                }
                Ok(false)
            }
            Command::Playback(command) => {
                self.action_delegate.send_playback_command(command);
                Ok(false)
            }
            Command::AddFeed(source) => {
                self.action_delegate
                    .send_feed_update_request(FeedUpdateRequest::AddFeed(source));
                Ok(false)
            }
            Command::DeleteFeed => {
                if let Some(selected_feed) = self.feeds_list.selection() {
                    self.action_delegate
                        .send_feed_update_request(FeedUpdateRequest::DeleteFeed(selected_feed.id));
                }
                Ok(false)
            }
            Command::Update => {
                if let Some(selected_feed) = self.feeds_list.selection() {
                    self.action_delegate
                        .send_feed_update_request(FeedUpdateRequest::UpdateSingle(selected_feed.id))
                }
                Ok(false)
            }
            Command::SetOption(options_update) => {
                self.options.update(options_update);
                Ok(true)
            }
            Command::SetFeedEnabled(enabled) => {
                if let Some(selected_feed) = self.feeds_list.selection() {
                    self.action_delegate.send_feed_update_request(
                        FeedUpdateRequest::SetFeedEnabled(selected_feed.id, enabled),
                    );
                }
                Ok(false)
            }
            Command::SetNew(is_new) => {
                if let Some(selected) = self.episodes_list.selection() {
                    match selected.status {
                        EpisodeSummaryStatus::New if !is_new => {
                            self.episodes_list.update_selection(|summary| {
                                summary.status = EpisodeSummaryStatus::NotStarted
                            });
                            Ok(true)
                        }
                        EpisodeSummaryStatus::NotStarted if is_new => {
                            self.episodes_list.update_selection(|summary| {
                                summary.status = EpisodeSummaryStatus::New
                            });
                            Ok(true)
                        }
                        _ => Ok(false),
                    }
                } else {
                    Ok(false)
                }
            }
            Command::UpdateAll => {
                self.action_delegate
                    .send_feed_update_request(FeedUpdateRequest::UpdateAll);
                Ok(false)
            }
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

    pub(crate) fn init_rc(&mut self) {
        let resolver = FileResolver::new();
        resolver.visit_all("rc", |path| {
            match self.handle_command(Command::Exec(path.to_path_buf())) {
                Ok(_) => false,
                Err(status) => {
                    if status.severity() == Severity::Error {
                        self.status = Some(status);
                        true
                    } else {
                        false
                    }
                }
            }
        });
    }

    pub(crate) fn set_episodes_list_data(
        &mut self,
        data: Versioned<PaginatedDataMessage<EpisodeSummary>>,
    ) -> bool {
        self.episodes_list.handle_data(data)
    }

    fn update_current_feed(&mut self) {
        let selected_id = self.feeds_list.selection().map(|item| item.id);
        if selected_id == self.selected_feed {
            return;
        }

        self.episodes_list.update_provider(|provider| {
            provider.query = selected_id.map(|selected_id| EpisodesQuery::Multiple {
                feed_id: Some(selected_id),
            })
        });
        self.selected_feed = selected_id;
    }

    pub(crate) fn set_feeds_list_data(&mut self, data: Versioned<Vec<FeedSummary>>) -> bool {
        if self.feeds_list.handle_data(data) {
            self.update_current_feed();
            true
        } else {
            false
        }
    }

    pub(crate) fn handle_player_notification(&mut self, notification: PlayerNotification) {
        match notification {
            PlayerNotification::VolumeChanged(volume) => {
                self.status = Some(Status::VolumeChanged(volume))
            }
            PlayerNotification::StateChanged(state) => {
                self.playback_state.set_state(state);
                if state.is_none() {
                    self.playing_episode = None;
                }
            }
            PlayerNotification::DurationSet(duration) => self.playback_state.set_duration(duration),
            PlayerNotification::PositionSet(position) => self.playback_state.set_position(position),
        }
    }

    pub(crate) fn handle_update_notification(&mut self, notification: FeedUpdateNotification) {
        match notification {
            FeedUpdateNotification::UpdateStarted(ids) => self.updating_feeds.extend(ids),
            FeedUpdateNotification::UpdateFinished(id, result) => {
                self.updating_feeds.remove(&id);
                match result {
                    FeedUpdateResult::Updated(summary) => self.feeds_list.replace_item(summary),
                    FeedUpdateResult::StatusChanged(status) => self
                        .feeds_list
                        .update_item(id, |summary| summary.status = status),
                }
                if self.selected_feed == Some(id) {
                    self.episodes_list.invalidate();
                }
            }
            FeedUpdateNotification::Error(error) => {
                self.status = Some(Status::new_custom(error.to_string(), Severity::Error));
            }
            FeedUpdateNotification::FeedAdded(feed) => {
                self.feeds_list.add_item(feed);
                self.update_current_feed();
            }
            FeedUpdateNotification::FeedDeleted(feed_id) => {
                self.feeds_list.remove_item(feed_id);
                self.update_current_feed();
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, CmdParsable)]
pub(crate) enum Command {
    Cursor(CursorCommand),
    Map(Key, Box<Command>),
    MapState(Key, FocusedPane, Box<Command>),
    Unmap(Key),
    UnmapState(Key, FocusedPane),
    Theme(ThemeCommand),
    Exec(PathBuf),
    Volume(VolumeCommand),
    PlayCurrent,
    Playback(PlaybackCommand),
    SetFeedEnabled(bool),
    #[cmd(alias = "q")]
    Quit,
    SetFocus(FocusedPane),
    SetOption(OptionsUpdate),
    AddFeed(String),
    DeleteFeed,
    Update,
    UpdateAll,
    SetNew(bool),
}

#[cfg(test)]
mod tests {
    use super::{ActionDelegate, Command, ViewModel};
    use crate::dataview::CursorCommand;
    use crate::theming::{List, StatusBar};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;
    use tempfile::tempdir;
    use tui::style::{Color, Style};

    fn write_file(dir: impl AsRef<Path>, filename: impl AsRef<Path>, content: &str) {
        let mut path = dir.as_ref().to_path_buf();
        path.push(filename);

        let mut file = File::create(path).unwrap();
        write!(file, "{}", content).unwrap();
    }

    struct NoopPlayerDelagate;

    impl ActionDelegate for NoopPlayerDelagate {
        fn start_playback(&self, _episode_id: hedgehog_library::model::EpisodeId) {}
        fn send_volume_command(&self, _command: hedgehog_player::volume::VolumeCommand) {}
        fn send_playback_command(&self, _command: hedgehog_player::PlaybackCommand) {}
        fn send_feed_update_request(&self, _command: hedgehog_library::FeedUpdateRequest) {}
    }

    #[test]
    fn init_rc() {
        let global_data = tempdir().unwrap();
        let user_data = tempdir().unwrap();
        let env_path = std::env::join_paths([global_data.path(), user_data.path()]).unwrap();
        // TODO: changes global state, environment variable cannot be set by multiple tests
        std::env::set_var("HEDGEHOG_PATH", env_path);

        write_file(
            global_data.path(),
            "rc",
            "Map Up Cursor Previous\nMap Down Cursor Next\nTheme Load default",
        );
        write_file(
            global_data.path(),
            "default.theme",
            "Load another NoReset\nSet statusbar.empty bg:red",
        );
        write_file(
            global_data.path(),
            "another.theme",
            "Set list.divider bg:blue",
        );

        write_file(
            user_data.path(),
            "another.theme",
            "Set statusbar.command bg:yellow",
        );
        write_file(user_data.path(), "rc", "Map Down Cursor Last");

        let mut view_model = ViewModel::new((32, 32), NoopPlayerDelagate);
        view_model.init_rc();

        assert!(view_model.status.is_none());
        assert_eq!(
            view_model
                .key_mapping
                .get(
                    KeyEvent::new(KeyCode::Up, KeyModifiers::empty()).into(),
                    None
                )
                .unwrap(),
            &Command::Cursor(CursorCommand::Previous)
        );
        assert_eq!(
            view_model
                .key_mapping
                .get(
                    KeyEvent::new(KeyCode::Down, KeyModifiers::empty()).into(),
                    None
                )
                .unwrap(),
            &Command::Cursor(CursorCommand::Last)
        );

        assert_eq!(
            view_model.theme.get(StatusBar::Command),
            Style::default().bg(Color::Yellow)
        );
        assert_eq!(
            view_model.theme.get(StatusBar::Empty),
            Style::default().bg(Color::Red)
        );
        assert_eq!(view_model.theme.get(List::Divider), Style::default());
    }
}
