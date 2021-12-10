use crate::cmdreader::{CommandReader, FileResolver};
use crate::events::key;
use crate::history::CommandsHistory;
use crate::keymap::{Key, KeyMapping};
use crate::mouse::{MouseEventKind, MouseHitResult, MouseState, WidgetPositions};
use crate::options::{Options, OptionsUpdate};
use crate::scrolling::pagination::{DataProvider, PaginatedData};
use crate::scrolling::{selection, DataView, ScrollAction, ScrollableList};
use crate::status::{HedgehogError, Severity, Status, StatusLog};
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
use directories::BaseDirs;
use hedgehog_library::datasource::QueryError;
use hedgehog_library::model::{
    EpisodePlaybackData, EpisodeStatus, EpisodeSummary, EpisodeSummaryStatus, EpisodesListMetadata,
    FeedId, FeedSummary, FeedView, Identifiable,
};
use hedgehog_library::search::{self, SearchClient, SearchResult};
use hedgehog_library::status_writer::{StatusWriter, StatusWriterCommand};
use hedgehog_library::{
    EpisodePlaybackDataRequest, EpisodeSummariesRequest, EpisodesListMetadataRequest,
    EpisodesQuery, FeedSummariesRequest, FeedUpdateNotification, FeedUpdateRequest,
    FeedUpdateResult, Library,
};
use hedgehog_player::state::PlaybackState;
use hedgehog_player::volume::VolumeCommand;
use hedgehog_player::{
    PlaybackCommand, PlaybackMetadata, Player, PlayerErrorNotification, PlayerNotification,
    SeekDirection,
};
use std::collections::HashSet;
use std::io::{stdout, Write};
use std::ops::Range;
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
    Search,
}

pub(crate) enum SearchState {
    Loading,
    Loaded(ScrollableList<Vec<SearchResult>>),
    Error(search::Error),
}

pub(crate) struct LibraryViewModel {
    pub(crate) feeds: ScrollableList<Vec<FeedView<FeedSummary>>>,
    pub(crate) feeds_loaded: bool,
    pub(crate) episodes: ScrollableList<PaginatedData<EpisodeSummary>>,
    pub(crate) episodes_list_metadata: Option<EpisodesListMetadata>,
    pub(crate) search: SearchState,
    pub(crate) focus: FocusedPane,
    pub(crate) updating_feeds: HashSet<FeedId>,
    pub(crate) playing_episode: Option<EpisodePlaybackData>,
}

impl LibraryViewModel {
    fn new(window_size: usize) -> Self {
        LibraryViewModel {
            feeds: ScrollableList::new(Vec::new(), window_size, 3),
            feeds_loaded: false,
            episodes: ScrollableList::new(PaginatedData::new(), window_size, 3),
            episodes_list_metadata: None,
            search: SearchState::Loading,
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
    Cursor(ScrollAction),
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
    Update {
        #[cmd(attr(this = "true"))]
        current_only: bool,
    },
    Mark {
        status: EpisodeStatus,
        #[cmd(attr(all = "true"))]
        update_all: bool,
    },
    Search(String),
    SearchAdd,

    Refresh,
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
    layout: WidgetPositions,
    mouse_state: MouseState,

    library_actor: Addr<Library>,
    player_actor: Addr<Player>,
    status_writer_actor: Addr<StatusWriter>,

    options: Options,
    theme: Theme,
    key_mapping: KeyMapping<Command, FocusedPane>,
    library: LibraryViewModel,
    selected_feed: Option<FeedView<FeedId>>,
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
            layout: WidgetPositions::default(),
            mouse_state: MouseState::default(),
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
        self.layout = WidgetPositions::default();
        let draw = |f: &mut tui::Frame<CrosstermBackend<std::io::Stdout>>| {
            let area = f.size();
            let (area, status_area) = split_bottom(area, 1);
            let (area, player_area) = split_bottom(area, 1);
            self.layout.set_player_status(player_area);

            let library_widget =
                LibraryWidget::new(&self.library, &self.options, &self.theme, &mut self.layout);
            f.render_widget(library_widget, area);

            let player_widget = PlayerState::new(
                &self.playback_state,
                &self.theme,
                &self.options,
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
        let mut title = String::new();
        let episode_title = playing_episode.and_then(|episode| episode.episode_title.as_deref());
        if let Some(episode_title) = episode_title {
            <String as std::fmt::Write>::write_fmt(
                &mut title,
                format_args!("{} | ", episode_title),
            )
            .unwrap();
        }
        let feed_title = playing_episode.and_then(|episode| episode.feed_title.as_deref());
        if let Some(feed_title) = feed_title {
            <String as std::fmt::Write>::write_fmt(&mut title, format_args!("{} | ", feed_title))
                .unwrap();
        }
        <String as std::fmt::Write>::write_str(&mut title, "hedgehog").unwrap();

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
                match &mut self.library.focus {
                    FocusedPane::FeedsList => {
                        self.library.feeds.scroll(command);
                        self.update_current_feed(ctx);
                    }
                    FocusedPane::EpisodesList => {
                        self.library.episodes.scroll(command);
                    }
                    FocusedPane::Search => {
                        if let SearchState::Loaded(list) = &mut self.library.search {
                            list.scroll(command);
                        }
                    }
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
                        if let Some(playback_data) = actor.handle_response_error(result, ctx) {
                            actor.library.playing_episode = Some(playback_data.clone());
                            actor.playback_state = PlaybackState::new_started(
                                playback_data.position,
                                playback_data.duration,
                            );
                            actor
                                .player_actor
                                .do_send(hedgehog_player::PlaybackCommand::Play(
                                    playback_data.media_url,
                                    playback_data.position,
                                    Some(PlaybackMetadata {
                                        episode_id: playback_data.id.as_i64(),
                                        episode_title: playback_data.episode_title,
                                        feed_title: playback_data.feed_title,
                                    }),
                                ));
                            actor
                                .library
                                .episodes
                                .update_data::<selection::DoNotUpdate, _>(|data| {
                                    let episode = data
                                        .find(|item| item.id == episode_id)
                                        .and_then(|index| data.item_at_mut(index));
                                    if let Some(episode) = episode {
                                        episode.status = EpisodeSummaryStatus::Started;
                                    }
                                });
                            actor.invalidate(ctx);
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
                if let Some(FeedView::Feed(selected_feed)) = self.library.feeds.selection() {
                    self.library_actor
                        .do_send(FeedUpdateRequest::DeleteFeed(selected_feed.id));
                }
            }
            Command::Update { current_only } => {
                if current_only {
                    if let Some(FeedView::Feed(selected_feed)) = self.selected_feed {
                        self.library_actor
                            .do_send(FeedUpdateRequest::UpdateSingle(selected_feed));
                    }
                } else {
                    self.library_actor.do_send(FeedUpdateRequest::UpdateAll);
                }
            }
            Command::SetOption(options_update) => {
                self.options.update(options_update);
                self.invalidate(ctx);
            }
            Command::SetFeedEnabled(enabled) => {
                if let Some(FeedView::Feed(selected_feed)) = self.selected_feed {
                    self.library_actor
                        .do_send(FeedUpdateRequest::SetFeedEnabled(selected_feed, enabled));
                }
            }
            Command::Mark { status, update_all } => {
                if update_all {
                    if let Some(feed) = self.selected_feed {
                        self.library
                            .episodes
                            .update_data::<selection::DoNotUpdate, _>(|data| {
                                for episode in data.iter_mut() {
                                    episode.status = status.clone().into();
                                }
                            });
                        let query = EpisodesQuery::from_feed_view(feed);
                        self.status_writer_actor
                            .do_send(StatusWriterCommand::Set(query, status));
                    }
                } else if let Some(selected_id) =
                    self.library.episodes.selection().map(|episode| episode.id)
                {
                    let selected_index = self.library.episodes.viewport().selected_index();
                    self.library
                        .episodes
                        .update_data::<selection::DoNotUpdate, _>(|data| {
                            if let Some(selected) = data.item_at_mut(selected_index) {
                                selected.status = status.clone().into();
                            }
                        });
                    self.status_writer_actor
                        .do_send(StatusWriterCommand::set(selected_id, status));
                }
                self.invalidate(ctx);
            }
            Command::Search(query) => {
                self.perform_search(query, ctx);
                self.library.focus = FocusedPane::Search;
                self.invalidate(ctx);
            }
            Command::SearchAdd => {
                if let SearchState::Loaded(list) = &self.library.search {
                    let index = list.viewport().selected_index();
                    if let Some(item) = list.data().item_at(index) {
                        let url = item.feed_url.clone();
                        self.library.focus = FocusedPane::FeedsList;
                        self.handle_command(Command::AddFeed(url), ctx);
                        self.invalidate(ctx);
                    }
                }
            }
            Command::Refresh => {
                self.load_feeds(ctx);
                self.library
                    .episodes
                    .update_data::<selection::Reset, _>(|data| {
                        data.clear_provider();
                        data.clear();
                    });
                self.invalidate(ctx);
            }
        }
    }

    fn init_rc(&mut self, ctx: &mut <UI as Actor>::Context) {
        let resolver = FileResolver::new();
        resolver.visit_all("rc", |path| {
            self.handle_command(Command::Exec(path.to_path_buf()), ctx);
            self.status.has_errors()
        });
    }

    fn refresh_episodes(&mut self, ctx: &mut <UI as Actor>::Context, replace_current: bool) {
        let feed_id = match self.selected_feed {
            Some(feed_id) => feed_id,
            None => return,
        };
        self.library
            .episodes
            .update_data::<selection::Keep, _>(|data| {
                // To prevent updates for the old
                data.clear_provider();
                if replace_current {
                    data.clear();
                }
            });

        let query = EpisodesQuery::from_feed_view(feed_id);
        let new_provider = EpisodesListProvider {
            query: query.clone(),
            actor: ctx.address(),
        };
        let future = wrap_future(
            self.library_actor
                .send(EpisodesListMetadataRequest(query.clone())),
        )
        .then(|result, actor: &mut UI, ctx| {
            let result = actor.handle_response_error(result, ctx).map(|metadata| {
                let range = actor.library.episodes.data().initial_range(
                    metadata.items_count,
                    actor.library.episodes.viewport().range(),
                );
                (metadata, range)
            });
            let library_actor = actor.library_actor.clone();
            wrap_future(async move {
                match result {
                    None => None,
                    Some((metadata, None)) => Some((metadata, None)),
                    Some((metadata, Some(range))) => {
                        let episodes = library_actor
                            .send(EpisodeSummariesRequest::new(query.clone(), range.clone()))
                            .await;
                        Some((metadata, Some((range, episodes))))
                    }
                }
            })
        })
        .map(move |result, actor: &mut UI, ctx| {
            macro_rules! update_data {
                ($fn:expr) => {{
                    let episodes = &mut actor.library.episodes;
                    if replace_current {
                        episodes.update_data::<selection::Reset, _>($fn);
                    } else {
                        episodes.update_data::<selection::FindPrevious, _>($fn);
                    }
                }};
            }
            if let Some((metadata, episodes)) = result {
                let items_count = metadata.items_count;
                actor.library.episodes_list_metadata = Some(metadata);
                match episodes {
                    Some((range, episodes)) => {
                        if let Some(episodes) = actor.handle_response_error(episodes, ctx) {
                            update_data!(|data| {
                                data.set_provider(new_provider);
                                data.set_initial(items_count, episodes, range);
                            });
                        }
                    }
                    None => {
                        update_data!(|data| {
                            data.set_provider(new_provider);
                            data.clear();
                        });
                    }
                }
                actor.invalidate(ctx);
            }
        });
        ctx.spawn(future);
    }

    fn update_current_feed(&mut self, ctx: &mut <UI as Actor>::Context) {
        let selected_id = self.library.feeds.selection().map(|item| item.id());
        if selected_id == self.selected_feed {
            return;
        }
        self.selected_feed = selected_id;

        if selected_id.is_some() {
            self.refresh_episodes(ctx, true);
        } else {
            self.library
                .episodes
                .update_data::<selection::Reset, _>(|data| {
                    data.clear();
                    data.clear_provider();
                });
        }
        self.invalidate(ctx);
    }

    fn perform_search(&mut self, query: String, ctx: &mut <UI as Actor>::Context) {
        self.library.search = SearchState::Loading;
        self.invalidate(ctx);

        let client = SearchClient::new();
        ctx.spawn(
            wrap_future(async move { client.perform(&query).await }).map(
                move |result, actor: &mut UI, ctx| {
                    actor.library.search = match result {
                        Ok(results) => SearchState::Loaded(ScrollableList::new(
                            results,
                            actor.library.feeds.viewport().window_size() / 2,
                            1,
                        )),
                        Err(err) => SearchState::Error(err),
                    };
                    actor.invalidate(ctx);
                },
            ),
        );
    }

    fn handle_error(
        &mut self,
        error: impl HedgehogError + 'static,
        ctx: &mut <UI as Actor>::Context,
    ) {
        self.status.push(Status::error(error));
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

    fn load_feeds(&mut self, ctx: &mut <UI as Actor>::Context) {
        ctx.spawn(
            self.library_actor
                .send(FeedSummariesRequest)
                .into_actor(self)
                .map(move |data, actor, ctx| {
                    if let Some(data) = actor.handle_response_error(data, ctx) {
                        actor
                            .library
                            .feeds
                            .update_data::<selection::FindPrevious, _>(|current_feeds| {
                                let mut feeds = Vec::with_capacity(data.len() + 1);
                                feeds.push(FeedView::All);
                                feeds.extend(data.into_iter().map(FeedView::Feed));
                                *current_feeds = feeds;
                            });
                        actor.update_current_feed(ctx);
                        actor.library.feeds_loaded = true;
                        actor.invalidate(ctx);
                    }
                }),
        );
    }
}

impl Actor for UI {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.load_feeds(ctx);
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

        if let Some(dirs) = BaseDirs::new() {
            let mut data_dir = dirs.data_local_dir().to_path_buf();
            data_dir.push("hedgehog");
            let result = std::fs::create_dir_all(&data_dir).and_then(|_| {
                data_dir.push("history");
                self.commands_history.load_file(data_dir)
            });
            if let Err(error) = result {
                self.handle_error(error, ctx);
            }
        }

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
            None if self.confirmation.is_none() => match event {
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
                crossterm::event::Event::Mouse(event) => {
                    let event = match self.mouse_state.handle_event(event) {
                        Some(event) => event,
                        None => return,
                    };
                    let widget = match self.layout.hit_test_at(event.row, event.column) {
                        Some(widget) => widget,
                        None => return,
                    };

                    match event.kind {
                        MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                            let offset = if event.kind == MouseEventKind::ScrollUp {
                                ScrollAction::MoveBy(-3)
                            } else {
                                ScrollAction::MoveBy(3)
                            };
                            match widget {
                                MouseHitResult::FeedsRow(_) => {
                                    self.library.feeds.scroll(offset);
                                    self.update_current_feed(ctx);
                                    self.library.focus = FocusedPane::FeedsList;
                                }
                                MouseHitResult::EpisodesRow(_) => {
                                    self.library.episodes.scroll(offset);
                                    self.library.focus = FocusedPane::EpisodesList;
                                }
                                MouseHitResult::SearchRow(_) => {
                                    if let SearchState::Loaded(ref mut list) = self.library.search {
                                        list.scroll(offset);
                                    }
                                }
                                MouseHitResult::Player => {
                                    let seek_direction = if event.kind == MouseEventKind::ScrollUp {
                                        SeekDirection::Forward
                                    } else {
                                        SeekDirection::Backward
                                    };
                                    self.handle_command(
                                        Command::Playback(PlaybackCommand::SeekRelative(
                                            Duration::from_secs(1),
                                            seek_direction,
                                        )),
                                        ctx,
                                    );
                                }
                            }
                            self.invalidate(ctx);
                        }
                        MouseEventKind::Click(is_double) => {
                            match widget {
                                MouseHitResult::FeedsRow(row) => {
                                    self.library.focus = FocusedPane::FeedsList;
                                    self.library.feeds.scroll(ScrollAction::MoveToVisible(row));
                                    self.update_current_feed(ctx);
                                }
                                MouseHitResult::EpisodesRow(row) => {
                                    self.library.focus = FocusedPane::EpisodesList;
                                    let valid = self
                                        .library
                                        .episodes
                                        .scroll(ScrollAction::MoveToVisible(row));
                                    if valid && is_double {
                                        self.handle_command(Command::PlayCurrent, ctx);
                                    }
                                }
                                MouseHitResult::SearchRow(row) => {
                                    self.library.focus = FocusedPane::Search;
                                    if let SearchState::Loaded(ref mut list) = self.library.search {
                                        let valid = list.scroll(ScrollAction::MoveToVisible(row));
                                        if valid && is_double {
                                            self.handle_command(Command::SearchAdd, ctx);
                                        }
                                    }
                                }
                                MouseHitResult::Player => {
                                    self.handle_command(
                                        Command::Playback(PlaybackCommand::TogglePause),
                                        ctx,
                                    );
                                }
                            }
                            self.invalidate(ctx);
                        }
                    }
                }
                _ => (),
            },
            None => match event {
                key!('y') | key!('Y', SHIFT) => {
                    let confirmation = self.confirmation.take().unwrap();
                    self.handle_command(confirmation.action, ctx);
                    self.invalidate(ctx);
                }
                key!('n') | key!('N', SHIFT) => {
                    self.confirmation = None;
                    self.invalidate(ctx);
                }
                key!(Enter) => {
                    let confirmation = self.confirmation.take().unwrap();
                    if confirmation.default {
                        self.handle_command(confirmation.action, ctx);
                    }
                    self.invalidate(ctx);
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
                        if let Err(error) = self.commands_history.push(&command_str) {
                            self.handle_error(error, ctx);
                        }
                        self.command = None;
                        match Command::parse_cmd_full(&command_str) {
                            Ok(command) => self.handle_command(command, ctx),
                            Err(error) => self.handle_error(error.into_static(), ctx),
                        }
                        self.invalidate(ctx);
                    }
                }
            }
        }
    }
}

pub(crate) struct EpisodesListProvider {
    query: EpisodesQuery,
    actor: Addr<UI>,
}

impl DataProvider for EpisodesListProvider {
    fn request(&self, range: std::ops::Range<usize>) {
        self.actor
            .do_send(DataFetchingRequest::Episodes(self.query.clone(), range));
    }
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
enum DataFetchingRequest {
    Episodes(EpisodesQuery, Range<usize>),
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
            DataFetchingRequest::Episodes(query, range) => {
                let request = EpisodeSummariesRequest::new(query, range.clone());
                Box::pin(self.library_actor.send(request).into_actor(self).map(
                    move |data, actor, ctx| {
                        if let Some(episodes) = actor.handle_response_error(data, ctx) {
                            actor
                                .library
                                .episodes
                                .update_data::<selection::DoNotUpdate, _>(|data| {
                                    data.set(episodes, range);
                                });
                            actor.invalidate(ctx);
                        }
                    },
                ))
            }
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
                    self.library.playing_episode.take();
                }
                self.invalidate(ctx);
            }
            PlayerNotification::DurationSet(duration) => {
                self.playback_state.set_duration(duration);
                self.invalidate(ctx);
            }
            PlayerNotification::PositionSet { position, .. } => {
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
            PlayerNotification::Eos => {
                if let Some(playing_episode) = &self.library.playing_episode {
                    self.status_writer_actor
                        .do_send(StatusWriterCommand::set_finished(playing_episode.id));
                    self.library
                        .episodes
                        .update_data::<selection::DoNotUpdate, _>(|data| {
                            let episode = data
                                .find(|item| item.id == playing_episode.id)
                                .and_then(|index| data.item_at_mut(index));
                            if let Some(episode) = episode {
                                episode.status = EpisodeSummaryStatus::Finished;
                            }
                        });
                }
            }
            PlayerNotification::MetadataChanged(_) => {}
        }
    }
}

impl Handler<PlayerErrorNotification> for UI {
    type Result = ();

    fn handle(&mut self, msg: PlayerErrorNotification, ctx: &mut Self::Context) -> Self::Result {
        self.handle_error(msg.0, ctx);
        if let Some(playing_episode) = self.library.playing_episode.take() {
            self.status_writer_actor
                .do_send(StatusWriterCommand::set_error(
                    playing_episode.id,
                    self.playback_state
                        .timing()
                        .map(|timing| timing.position)
                        .unwrap_or_default(),
                ));
            self.library
                .episodes
                .update_data::<selection::DoNotUpdate, _>(|data| {
                    let episode = data
                        .find(|item| item.id == playing_episode.id)
                        .and_then(|index| data.item_at_mut(index));
                    if let Some(episode) = episode {
                        episode.status = EpisodeSummaryStatus::Error;
                    }
                });
            self.invalidate(ctx);
        }
    }
}

impl Handler<FeedUpdateNotification> for UI {
    type Result = ();

    fn handle(&mut self, msg: FeedUpdateNotification, ctx: &mut Self::Context) -> Self::Result {
        match msg {
            FeedUpdateNotification::UpdateStarted(ids) => self.library.updating_feeds.extend(ids),
            FeedUpdateNotification::UpdateFinished(id, result) => {
                self.library.updating_feeds.remove(&id);
                self.library
                    .feeds
                    .update_data::<selection::DoNotUpdate, _>(|feeds| {
                        let item = feeds
                            .iter_mut()
                            .find(|feed| feed.id() == FeedView::Feed(id));
                        let item = match item {
                            Some(item) => item,
                            None => return,
                        };
                        match result {
                            FeedUpdateResult::Updated(summary) => *item = FeedView::Feed(summary),
                            FeedUpdateResult::StatusChanged(status) => {
                                item.as_mut().unwrap().status = status;
                            }
                        }
                    });
                if self.selected_feed == Some(FeedView::Feed(id))
                    || self.selected_feed == Some(FeedView::All)
                {
                    self.refresh_episodes(ctx, false);
                }
            }
            FeedUpdateNotification::Error(error) => self.handle_error(error, ctx),
            FeedUpdateNotification::FeedAdded(feed) => {
                self.library
                    .feeds
                    .update_data::<selection::Keep, _>(|feeds| feeds.push(FeedView::Feed(feed)));
                self.update_current_feed(ctx);
            }
            FeedUpdateNotification::FeedDeleted(feed_id) => {
                self.library
                    .feeds
                    .update_data::<selection::FindPrevious, _>(|feeds| {
                        let index = feeds.iter().enumerate().find_map(|(index, feed)| {
                            match feed.id() == FeedView::Feed(feed_id) {
                                true => Some(index),
                                false => None,
                            }
                        });
                        if let Some(index) = index {
                            feeds.remove(index);
                        }
                    });
                self.update_current_feed(ctx);
            }
        }
        self.invalidate(ctx);
    }
}
