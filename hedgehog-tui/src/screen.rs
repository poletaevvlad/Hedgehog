use crate::cmdreader::{CommandReader, FileResolver};
use crate::dataview::{
    CursorCommand, DataProvider, InteractiveList, ListData, ListDataRequest, PaginatedData,
    PaginatedDataMessage, PaginatedDataRequest, Versioned,
};
use crate::events::key;
use crate::history::CommandsHistory;
use crate::keymap::{Key, KeyMapping};
use crate::options::{Options, OptionsUpdate};
use crate::status::{Severity, Status, StatusLog};
use crate::theming::{Theme, ThemeCommand};
use crate::widgets::command::{CommandActionResult, CommandEditor, CommandState};
use crate::widgets::confirmation::ConfirmationView;
use crate::widgets::library::LibraryWidget;
use crate::widgets::player_state::PlayerState;
use crate::widgets::split_bottom;
use crate::widgets::status::StatusView;
use actix::clock::sleep;
use actix::fut::wrap_future;
use actix::prelude::*;
use cmd_parser::CmdParsable;
use crossterm::event::Event;
use crossterm::{terminal, QueueableCommand};
use hedgehog_library::datasource::QueryError;
use hedgehog_library::model::{EpisodeSummary, EpisodesListMetadata, FeedId, FeedSummary};
use hedgehog_library::status_writer::{StatusWriter, StatusWriterCommand};
use hedgehog_library::{
    EpisodePlaybackDataRequest, EpisodeSummariesRequest, EpisodesListMetadataRequest,
    EpisodesQuery, FeedSummariesRequest, FeedUpdateNotification, FeedUpdateRequest,
    FeedUpdateResult, Library,
};
use hedgehog_player::state::PlaybackState;
use hedgehog_player::volume::VolumeCommand;
use hedgehog_player::{PlaybackCommand, Player, PlayerErrorNotification, PlayerNotification};
use std::collections::HashSet;
use std::io::{stdout, Write};
use std::path::PathBuf;
use std::time::Duration;
use tui::backend::CrosstermBackend;
use tui::Terminal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, CmdParsable)]
pub(crate) enum FocusedPane {
    #[cmd(rename = "feeds")]
    FeedsList,
    #[cmd(rename = "episodes")]
    EpisodesList,
}

pub(crate) struct LibraryViewModel {
    pub(crate) feeds: InteractiveList<ListData<FeedSummary>, FeedsListProvider>,
    pub(crate) episodes: InteractiveList<PaginatedData<EpisodeSummary>, EpisodesListProvider>,
    pub(crate) episodes_list_metadata: Option<EpisodesListMetadata>,
    pub(crate) focus: FocusedPane,
    pub(crate) updating_feeds: HashSet<FeedId>,
    pub(crate) playing_episode: Option<EpisodeSummary>,
}

impl LibraryViewModel {
    fn new(window_size: usize) -> Self {
        LibraryViewModel {
            feeds: InteractiveList::new(window_size),
            episodes: InteractiveList::new(window_size),
            episodes_list_metadata: None,
            focus: FocusedPane::FeedsList,
            playing_episode: None,
            updating_feeds: HashSet::new(),
        }
    }

    fn set_window_size(&mut self, window_size: usize) {
        self.episodes.set_window_size(window_size);
        self.feeds.set_window_size(window_size);
    }
}

#[derive(Debug, Clone, PartialEq, CmdParsable)]
pub(crate) enum Command {
    #[cmd(rename = "line")]
    Cursor(CursorCommand),
    Map(Key, #[cmd(attr(state))] Option<FocusedPane>, Box<Command>),
    Unmap(Key, #[cmd(attr(state))] Option<FocusedPane>),
    Theme(ThemeCommand),
    Exec(PathBuf),
    Confirm(Box<CommandConfirmation>),
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

#[derive(Debug, Clone, PartialEq, CmdParsable)]
pub(crate) struct CommandConfirmation {
    pub(crate) prompt: String,
    pub(crate) action: Command,
    #[cmd(attr(default))]
    pub(crate) default: bool,
}

pub(crate) struct UI {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    invalidation_request: Option<SpawnHandle>,
    status_clear_request: Option<SpawnHandle>,

    library_actor: Addr<Library>,
    player_actor: Addr<Player>,
    status_writer_actor: Addr<StatusWriter>,

    options: Options,
    theme: Theme,
    key_mapping: KeyMapping<Command, FocusedPane>,
    library: LibraryViewModel,
    selected_feed: Option<FeedId>,
    playback_state: PlaybackState,

    status: StatusLog,
    command: Option<CommandState>,
    commands_history: CommandsHistory,
    confirmation: Option<CommandConfirmation>,
}

impl UI {
    pub(crate) fn new(
        size: (u16, u16),
        terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
        library_actor: Addr<Library>,
        player_actor: Addr<Player>,
        status_writer_actor: Addr<StatusWriter>,
    ) -> Self {
        UI {
            terminal,
            invalidation_request: None,
            status_clear_request: None,
            library_actor,
            player_actor,
            status_writer_actor,

            options: Options::default(),
            theme: Theme::default(),
            key_mapping: KeyMapping::default(),
            library: LibraryViewModel::new(size.1.saturating_sub(2) as usize),
            selected_feed: None,
            playback_state: PlaybackState::default(),

            status: StatusLog::default(),
            command: None,
            commands_history: CommandsHistory::new(),
            confirmation: None,
        }
    }

    fn render(&mut self) {
        let draw = |f: &mut tui::Frame<CrosstermBackend<std::io::Stdout>>| {
            let area = f.size();
            let (area, status_area) = split_bottom(area, 1);
            let (area, player_area) = split_bottom(area, 1);

            let library_widget = LibraryWidget::new(&self.library, &self.options, &self.theme);
            f.render_widget(library_widget, area);

            let player_widget = PlayerState::new(
                &self.playback_state,
                &self.theme,
                self.library.playing_episode.as_ref(),
            );
            f.render_widget(player_widget, player_area);

            if let Some(ref mut command_state) = self.command {
                CommandEditor::new(command_state)
                    .prefix(":")
                    .theme(&self.theme)
                    .render(f, status_area, &self.commands_history);
            } else if let Some(ref confirmation) = self.confirmation {
                let confirmation = ConfirmationView::new(confirmation, &self.theme);
                f.render_widget(confirmation, status_area);
            } else {
                let status = StatusView::new(self.status.display_status(), &self.theme);
                f.render_widget(status, status_area);
            }
        };
        self.terminal.draw(draw).unwrap();

        let playing_episode = self.library.playing_episode.as_ref();
        let episode_title = playing_episode.and_then(|episode| episode.title.as_deref());
        let title = match episode_title {
            Some(title) => format!("{} | hedgehog", title),
            None => "hedgehog".to_string(),
        };
        let mut stdout = stdout();
        stdout.queue(terminal::SetTitle(title)).unwrap();
        stdout.flush().unwrap();
    }

    fn invalidate(&mut self, ctx: &mut <Self as Actor>::Context) {
        if let Some(handle) = self.invalidation_request.take() {
            ctx.cancel_future(handle);
        }
        let future = wrap_future(sleep(Duration::from_millis(1)))
            .map(|_result, actor: &mut UI, _ctx| actor.render());
        self.invalidation_request = Some(ctx.spawn(future));
    }

    fn handle_command(&mut self, command: Command, ctx: &mut <Self as Actor>::Context) {
        match command {
            Command::Cursor(command) => {
                match self.library.focus {
                    FocusedPane::FeedsList => {
                        self.library.feeds.handle_command(command);
                        self.update_current_feed(ctx);
                    }
                    FocusedPane::EpisodesList => self.library.episodes.handle_command(command),
                }
                self.invalidate(ctx);
            }
            Command::SetFocus(focused_pane) => {
                if self.library.focus != focused_pane {
                    self.library.focus = focused_pane;
                    self.invalidate(ctx);
                }
            }
            Command::Quit => System::current().stop(),
            Command::Map(key, state, command) => {
                let redefined = self.key_mapping.contains(key, state);
                self.key_mapping.map(key, state, *command);
                if redefined {
                    self.set_status(
                        Status::new_custom("Key mapping redefined", Severity::Information),
                        ctx,
                    );
                }
            }
            Command::Unmap(key, state) => {
                if !self.key_mapping.unmap(key, state) {
                    self.set_status(
                        Status::new_custom("Key mapping is not defined", Severity::Warning),
                        ctx,
                    );
                }
            }
            Command::Theme(command) => {
                if let Err(error) = self.theme.handle_command(command) {
                    self.handle_error(error, ctx);
                } else {
                    self.invalidate(ctx);
                }
            }
            Command::Exec(path) => {
                let mut reader = match CommandReader::open(path) {
                    Ok(reader) => reader,
                    Err(error) => {
                        self.handle_error(error, ctx);
                        return;
                    }
                };

                loop {
                    match reader.read() {
                        Ok(None) => break,
                        Ok(Some(command)) => {
                            self.handle_command(command, ctx);
                            if self.status.has_errors() {
                                return;
                            }
                        }
                        Err(error) => {
                            self.handle_error(error, ctx);
                            return;
                        }
                    }
                }
            }
            Command::Confirm(confirmation) => {
                self.confirmation = Some(*confirmation);
                self.invalidate(ctx);
            }
            Command::PlayCurrent => {
                let episode_id = if let Some(current_episode) = self.library.episodes.selection() {
                    let episode_id = current_episode.id;
                    if Some(episode_id)
                        == self
                            .library
                            .playing_episode
                            .as_ref()
                            .map(|episode| episode.id)
                    {
                        return;
                    }
                    self.library.playing_episode = Some(current_episode.clone());
                    episode_id
                } else {
                    return;
                };
                self.invalidate(ctx);

                let future = self
                    .library_actor
                    .send(EpisodePlaybackDataRequest(episode_id))
                    .into_actor(self)
                    .map(move |result, actor, ctx| {
                        let requested_playback = actor.library.playing_episode.as_ref();
                        if requested_playback.map(|episode| episode.id) != Some(episode_id) {
                            return;
                        }
                        if let Some(playback_data) = actor.handle_response_error(result, ctx) {
                            actor
                                .player_actor
                                .do_send(hedgehog_player::PlaybackCommand::Play(
                                    playback_data.media_url,
                                    playback_data.position,
                                ));
                        }
                    });
                ctx.spawn(future);
            }
            Command::Playback(command) => self.player_actor.do_send(command),
            Command::Volume(command) => self.player_actor.do_send(command),
            Command::AddFeed(source) => self
                .library_actor
                .do_send(FeedUpdateRequest::AddFeed(source)),
            Command::DeleteFeed => {
                if let Some(selected_feed) = self.library.feeds.selection() {
                    self.library_actor
                        .do_send(FeedUpdateRequest::DeleteFeed(selected_feed.id));
                }
            }
            Command::Update => {
                if let Some(selected_feed) = self.selected_feed {
                    self.library_actor
                        .do_send(FeedUpdateRequest::UpdateSingle(selected_feed));
                }
            }
            Command::SetOption(options_update) => {
                self.options.update(options_update);
                self.invalidate(ctx);
            }
            Command::SetFeedEnabled(enabled) => {
                if let Some(selected_feed) = self.selected_feed {
                    self.library_actor
                        .do_send(FeedUpdateRequest::SetFeedEnabled(selected_feed, enabled));
                }
            }
            Command::SetNew(_is_new) => {
                /*if let Some(selected) = self.library.episodes.selection() {
                    match selected.status {
                        EpisodeSummaryStatus::New if !is_new => {
                            self.library.episodes.update_selection(|summary| {
                                summary.status = EpisodeSummaryStatus::NotStarted;
                            });
                        }
                        EpisodeSummaryStatus::NotStarted if is_new => {
                            self.library.episodes.update_selection(|summary| {
                                summary.status = EpisodeSummaryStatus::New;
                            });
                        }
                        _ => (),
                    }
                }*/
            }
            Command::UpdateAll => self.library_actor.do_send(FeedUpdateRequest::UpdateAll),
        }
    }

    fn init_rc(&mut self, ctx: &mut <UI as Actor>::Context) {
        let resolver = FileResolver::new();
        resolver.visit_all("rc", |path| {
            self.handle_command(Command::Exec(path.to_path_buf()), ctx);
            self.status.has_errors()
        });
    }

    fn update_current_feed(&mut self, ctx: &mut <UI as Actor>::Context) {
        let selected_id = self.library.feeds.selection().map(|item| item.id);
        if selected_id == self.selected_feed {
            return;
        }

        self.library.episodes.update_provider(|provider| {
            provider.query = selected_id.map(|selected_id| EpisodesQuery::Multiple {
                feed_id: Some(selected_id),
            });
        });
        self.selected_feed = selected_id;
        self.invalidate(ctx);
    }

    fn handle_error(&mut self, error: impl std::error::Error, ctx: &mut <UI as Actor>::Context) {
        self.status
            .push(Status::new_custom(error.to_string(), Severity::Error));
        self.invalidate(ctx);
    }

    fn set_status(&mut self, status: Status, ctx: &mut <UI as Actor>::Context) {
        if let Some(handle) = self.status_clear_request.take() {
            ctx.cancel_future(handle);
        }
        if let Some(duration) = status.ttl() {
            self.status_clear_request = Some(ctx.spawn(wrap_future(sleep(duration)).map(
                |_, actor: &mut UI, ctx| {
                    actor.status_clear_request = None;
                    actor.status.clear_display();
                    actor.invalidate(ctx);
                },
            )));
        }
        self.status.push(status);
        self.invalidate(ctx);
    }

    fn clear_status(&mut self, ctx: &mut <UI as Actor>::Context) {
        self.status.clear_display();
        if let Some(handle) = self.status_clear_request.take() {
            ctx.cancel_future(handle);
        }
    }
}

impl Actor for UI {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.library.episodes.set_provider(EpisodesListProvider {
            query: None,
            actor: ctx.address(),
        });
        self.library.feeds.set_provider(FeedsListProvider {
            actor: ctx.address(),
        });
        self.init_rc(ctx);

        self.player_actor
            .do_send(hedgehog_player::ActorCommand::Subscribe(
                ctx.address().recipient(),
            ));
        self.player_actor
            .do_send(hedgehog_player::ActorCommand::SubscribeErrors(
                ctx.address().recipient(),
            ));
        self.library_actor
            .do_send(hedgehog_library::FeedUpdateRequest::Subscribe(
                ctx.address().recipient(),
            ));

        ctx.add_stream(crossterm::event::EventStream::new());
        self.invalidate(ctx);
    }
}

impl StreamHandler<crossterm::Result<crossterm::event::Event>> for UI {
    fn handle(
        &mut self,
        item: crossterm::Result<crossterm::event::Event>,
        ctx: &mut Self::Context,
    ) {
        let event = match item {
            Ok(Event::Resize(_, height)) => {
                self.library
                    .set_window_size(height.saturating_sub(2) as usize);
                self.invalidate(ctx);
                return;
            }
            Ok(event) => event,
            Err(_) => {
                System::current().stop();
                return;
            }
        };

        match self.command {
            None => match event {
                key!('c', CONTROL) => self.handle_command(Command::Quit, ctx),
                key!(':') => {
                    self.clear_status(ctx);
                    self.command = Some(CommandState::default());
                    self.invalidate(ctx);
                }
                crossterm::event::Event::Key(key_event) => {
                    let command = self
                        .key_mapping
                        .get(key_event.into(), Some(self.library.focus));
                    if let Some(command) = command.cloned() {
                        self.handle_command(command, ctx);
                    }
                }
                _ => (),
            },
            Some(ref mut command_state) => {
                match command_state.handle_event(event, &self.commands_history) {
                    CommandActionResult::None => (),
                    CommandActionResult::Update => self.invalidate(ctx),
                    CommandActionResult::Clear => {
                        self.command = None;
                        self.invalidate(ctx);
                    }
                    CommandActionResult::Submit => {
                        let command_str = command_state.as_str(&self.commands_history).to_string();
                        self.commands_history.push(&command_str);
                        self.command = None;
                        match Command::parse_cmd_full(&command_str) {
                            Ok(command) => self.handle_command(command, ctx),
                            Err(error) => self.handle_error(error, ctx),
                        }
                        self.invalidate(ctx);
                    }
                }
            }
        }
    }
}

pub(crate) struct EpisodesListProvider {
    pub(crate) query: Option<EpisodesQuery>,
    actor: Addr<UI>,
}

impl DataProvider for EpisodesListProvider {
    type Request = PaginatedDataRequest;

    fn request(&self, request: crate::dataview::Versioned<Self::Request>) {
        if let Some(query) = &self.query {
            self.actor
                .do_send(DataFetchingRequest::Episodes(query.clone(), request));
        }
    }
}

pub(crate) struct FeedsListProvider {
    actor: Addr<UI>,
}

impl DataProvider for FeedsListProvider {
    type Request = ListDataRequest;

    fn request(&self, request: Versioned<Self::Request>) {
        self.actor.do_send(DataFetchingRequest::Feeds(request));
    }
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
enum DataFetchingRequest {
    Episodes(EpisodesQuery, Versioned<PaginatedDataRequest>),
    Feeds(Versioned<ListDataRequest>),
}

type LibraryQueryResult<T> = Result<Result<T, QueryError>, MailboxError>;

impl UI {
    fn handle_response_error<T>(
        &mut self,
        data: LibraryQueryResult<T>,
        ctx: &mut <UI as Actor>::Context,
    ) -> Option<T> {
        match data {
            Err(err) => self.handle_error(err, ctx),
            Ok(Err(err)) => self.handle_error(err, ctx),
            Ok(Ok(data)) => return Some(data),
        }
        None
    }
}

impl Handler<DataFetchingRequest> for UI {
    type Result = ResponseActFuture<Self, ()>;

    fn handle(&mut self, msg: DataFetchingRequest, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            DataFetchingRequest::Episodes(query, request) => {
                let (version, request) = request.deconstruct();
                match request {
                    PaginatedDataRequest::Size => Box::pin(
                        self.library_actor
                            .send(EpisodesListMetadataRequest(query))
                            .into_actor(self)
                            .map(move |metadata, actor, ctx| {
                                if let Some(metadata) = actor.handle_response_error(metadata, ctx) {
                                    actor.library.episodes.handle_data(
                                        Versioned::new(PaginatedDataMessage::size(
                                            metadata.items_count,
                                        ))
                                        .with_version(version),
                                    );
                                    actor.library.episodes_list_metadata = Some(metadata);
                                    actor.invalidate(ctx);
                                }
                            }),
                    ),
                    PaginatedDataRequest::Page(page) => {
                        let page_index = page.index;
                        let request = EpisodeSummariesRequest::new(query, page);
                        Box::pin(self.library_actor.send(request).into_actor(self).map(
                            move |data, actor, ctx| {
                                if let Some(data) = actor.handle_response_error(data, ctx) {
                                    let message = PaginatedDataMessage::page(page_index, data);
                                    actor
                                        .library
                                        .episodes
                                        .handle_data(Versioned::new(message).with_version(version));
                                    actor.invalidate(ctx);
                                }
                            },
                        ))
                    }
                }
            }
            DataFetchingRequest::Feeds(request) => Box::pin(
                self.library_actor
                    .send(FeedSummariesRequest)
                    .into_actor(self)
                    .map(move |data, actor, ctx| {
                        if let Some(data) = actor.handle_response_error(data, ctx) {
                            actor
                                .library
                                .feeds
                                .handle_data(Versioned::new(data).with_version(request.version()));
                            actor.update_current_feed(ctx);
                            actor.invalidate(ctx);
                        }
                    }),
            ),
        }
    }
}

impl Handler<PlayerNotification> for UI {
    type Result = ();

    fn handle(&mut self, msg: PlayerNotification, ctx: &mut Self::Context) -> Self::Result {
        match msg {
            PlayerNotification::VolumeChanged(volume) => {
                self.set_status(Status::VolumeChanged(volume), ctx);
            }
            PlayerNotification::StateChanged(state) => {
                self.playback_state.set_state(state);
                if state.is_none() {
                    if let Some(playing_episode) = &self.library.playing_episode {
                        self.status_writer_actor
                            .do_send(StatusWriterCommand::set_finished(playing_episode.id));
                        self.library.playing_episode = None;
                    }
                }
                self.invalidate(ctx);
            }
            PlayerNotification::DurationSet(duration) => {
                self.playback_state.set_duration(duration);
                self.invalidate(ctx);
            }
            PlayerNotification::PositionSet(position) => {
                if let Some(playing_episode) = &self.library.playing_episode {
                    self.status_writer_actor
                        .do_send(StatusWriterCommand::set_position(
                            playing_episode.id,
                            position,
                        ));
                }
                self.playback_state.set_position(position);
                self.invalidate(ctx);
            }
        }
    }
}

impl Handler<PlayerErrorNotification> for UI {
    type Result = ();

    fn handle(&mut self, msg: PlayerErrorNotification, ctx: &mut Self::Context) -> Self::Result {
        self.handle_error(msg.0, ctx);
    }
}

impl Handler<FeedUpdateNotification> for UI {
    type Result = ();

    fn handle(&mut self, msg: FeedUpdateNotification, ctx: &mut Self::Context) -> Self::Result {
        match msg {
            FeedUpdateNotification::UpdateStarted(ids) => self.library.updating_feeds.extend(ids),
            FeedUpdateNotification::UpdateFinished(id, result) => {
                self.library.updating_feeds.remove(&id);
                match result {
                    FeedUpdateResult::Updated(summary) => self.library.feeds.replace_item(summary),
                    FeedUpdateResult::StatusChanged(status) => self
                        .library
                        .feeds
                        .update_item(id, |summary| summary.status = status),
                }
                if self.selected_feed == Some(id) {
                    self.library.episodes.invalidate();
                }
            }
            FeedUpdateNotification::Error(error) => self.handle_error(error, ctx),
            FeedUpdateNotification::FeedAdded(feed) => {
                self.library.feeds.add_item(feed);
                self.update_current_feed(ctx);
            }
            FeedUpdateNotification::FeedDeleted(feed_id) => {
                self.library.feeds.remove_item(feed_id);
                self.update_current_feed(ctx);
            }
        }
        self.invalidate(ctx);
    }
}
