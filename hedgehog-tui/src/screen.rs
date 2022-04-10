use crate::cmdreader::CommandReader;
use crate::events::key;
use crate::history::CommandsHistory;
use crate::keymap::{Key, KeyMapping};
use crate::logger::{LogEntry, LogHistory};
use crate::mouse::{MouseEventKind, MouseHitResult, MouseState, WidgetPositions};
use crate::options::{Options, OptionsUpdate};
use crate::scrolling::pagination::{DataProvider, PaginatedData};
use crate::scrolling::{selection, DataView, ScrollAction, ScrollableList};
use crate::theming::{Theme, ThemeCommand};
use crate::widgets::command::{CommandActionResult, CommandEditor, CommandState};
use crate::widgets::confirmation::ConfirmationView;
use crate::widgets::errors_log::ErrorsLogWidget;
use crate::widgets::library::LibraryWidget;
use crate::widgets::player_state::PlayerState;
use crate::widgets::search_results::SearchResults;
use crate::widgets::split_bottom;
use crate::widgets::status::LogEntryView;
use actix::clock::sleep;
use actix::fut::wrap_future;
use actix::prelude::*;
use crossterm::event::{self, Event};
use crossterm::QueueableCommand;
use hedgehog_library::datasource::QueryError;
use hedgehog_library::model::{
    Episode, EpisodePlaybackData, EpisodeStatus, EpisodeSummary, EpisodeSummaryStatus,
    EpisodesListMetadata, Feed, FeedId, FeedSummary, FeedView, Identifiable,
};
use hedgehog_library::search::{self, SearchClient, SearchResult};
use hedgehog_library::status_writer::{StatusWriter, StatusWriterCommand};
use hedgehog_library::{
    EpisodePlaybackDataRequest, EpisodeSummariesRequest, EpisodesListMetadataRequest,
    EpisodesQuery, FeedSummariesRequest, FeedUpdateNotification, FeedUpdateRequest,
    FeedUpdateResult, Library, NewFeedMetadata, UpdateQuery,
};
use hedgehog_player::state::PlaybackState;
use hedgehog_player::volume::VolumeCommand;
use hedgehog_player::{
    PlaybackCommand, PlaybackMetadata, Player, PlayerErrorNotification, PlayerNotification,
    SeekDirection, SeekOffset,
};
use std::collections::HashSet;
use std::io::{stdout, Write};
use std::ops::Range;
use std::path::PathBuf;
use std::time::Duration;
use tui::backend::CrosstermBackend;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, cmdparse::Parsable)]
pub(crate) enum FocusedPane {
    #[cmd(rename = "feeds")]
    FeedsList,
    #[cmd(rename = "episodes")]
    EpisodesList,
    Search,
    #[cmd(rename = "log")]
    ErrorsLog,
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

#[derive(Debug, Clone, PartialEq, cmdparse::Parsable)]
pub(crate) enum Command {
    #[cmd(rename = "line")]
    Cursor(ScrollAction),
    Map(Key, #[cmd(attr(state))] Option<FocusedPane>, Box<Command>),
    Unmap(Key, #[cmd(attr(state))] Option<FocusedPane>),
    Theme(ThemeCommand),
    Exec(PathBuf),
    Confirm(Box<CommandConfirmation>),
    #[cmd(transparent)]
    Volume(VolumeCommand),
    PlayCurrent,
    #[cmd(transparent)]
    Playback(PlaybackCommand),
    Finish,
    #[cmd(alias = "enable", alias = "disable")]
    SetFeedEnabled(
        #[cmd(
            alias_value(alias = "enable", value = "true"),
            alias_value(alias = "disable", value = "false")
        )]
        bool,
    ),
    #[cmd(alias = "q")]
    Quit,
    #[cmd(rename = "focus", alias = "log")]
    SetFocus(#[cmd(alias_value(alias = "log", value = "FocusedPane::ErrorsLog"))] FocusedPane),
    #[cmd(rename = "set")]
    SetOption(OptionsUpdate),
    #[cmd(rename = "add")]
    AddFeed(String),
    DeleteFeed,
    Reverse,
    #[cmd(alias = "u")]
    Update {
        #[cmd(attr(this = "true"))]
        current_only: bool,
    },
    Mark {
        status: EpisodeStatus,
        #[cmd(attr(all = "true"))]
        update_all: bool,
        #[cmd(attr(if))]
        condition: Option<EpisodeSummaryStatus>,
    },
    #[cmd(ignore, alias = "hide", alias = "unhide")]
    SetEpisodeHidden(
        #[cmd(
            alias_value(alias = "hide", value = "true"),
            alias_value(alias = "unhide", value = "false")
        )]
        bool,
    ),
    #[cmd(alias = "s")]
    Search(#[cmd(parser = "hedgehog_library::search::SearchQueryParser")] String),
    SearchAdd,
    OpenLink(LinkType),

    Refresh,
}

#[derive(Debug, Clone, PartialEq, cmdparse::Parsable)]
pub(crate) enum LinkType {
    Feed,
    Episode,
}

#[derive(Debug, Clone, PartialEq, cmdparse::Parsable)]
pub(crate) struct CommandConfirmation {
    pub(crate) prompt: String,
    pub(crate) action: Command,
    #[cmd(attr(default))]
    pub(crate) default: bool,
}

pub(crate) struct UI {
    app_env: super::AppEnvironment,

    terminal: tui::Terminal<CrosstermBackend<std::io::Stdout>>,
    invalidation_request: Option<SpawnHandle>,
    log_display_clear_request: Option<SpawnHandle>,
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

    log_history: ScrollableList<LogHistory>,
    command: Option<CommandState>,
    commands_history: CommandsHistory,
    confirmation: Option<CommandConfirmation>,
}

impl UI {
    pub(crate) fn new(
        size: (u16, u16),
        terminal: tui::Terminal<CrosstermBackend<std::io::Stdout>>,
        library_actor: Addr<Library>,
        player_actor: Addr<Player>,
        status_writer_actor: Addr<StatusWriter>,
        app_env: super::AppEnvironment,
    ) -> Self {
        UI {
            app_env,
            terminal,
            invalidation_request: None,
            layout: WidgetPositions::default(),
            mouse_state: MouseState::default(),
            log_display_clear_request: None,
            library_actor,
            player_actor,
            status_writer_actor,

            options: Options::default(),
            theme: Theme::default(),
            key_mapping: KeyMapping::default(),
            library: LibraryViewModel::new(size.1.saturating_sub(2) as usize),
            selected_feed: None,
            playback_state: PlaybackState::default(),

            log_history: ScrollableList::new(
                LogHistory::default(),
                size.1.saturating_sub(2) as usize / 3,
                1,
            ),
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
            self.layout.set_command_entry(status_area);
            self.layout.set_player_status(player_area);

            match self.library.focus {
                FocusedPane::FeedsList | FocusedPane::EpisodesList => {
                    let library_widget = LibraryWidget::new(
                        &self.library,
                        &self.options,
                        &self.theme,
                        &mut self.layout,
                    );
                    f.render_widget(library_widget, area);
                }
                FocusedPane::Search => {
                    self.layout.set_search_list(area);
                    let widget = SearchResults::new(&self.library.search, &self.theme);
                    f.render_widget(widget, area);
                }
                FocusedPane::ErrorsLog => {
                    let widget = ErrorsLogWidget::new(&self.log_history, &self.theme);
                    f.render_widget(widget, area);
                }
            }

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
                let status =
                    LogEntryView::new(self.log_history.data().display_entry(), &self.theme);
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
        stdout.queue(crossterm::terminal::SetTitle(title)).unwrap();
        stdout.flush().unwrap();
    }

    fn invalidate_later(&mut self, ctx: &mut <Self as Actor>::Context) {
        if let Some(handle) = self.invalidation_request.take() {
            ctx.cancel_future(handle);
        }
        let future = wrap_future(sleep(Duration::from_millis(1)))
            .map(|_result, actor: &mut UI, _ctx| actor.render());
        self.invalidation_request = Some(ctx.spawn(future));
    }

    fn invalidate(&mut self, ctx: &mut <Self as Actor>::Context) {
        if let Some(handle) = self.invalidation_request.take() {
            ctx.cancel_future(handle);
        }
        self.render();
    }

    fn handle_command(&mut self, command: Command, ctx: &mut <Self as Actor>::Context) {
        match command {
            Command::Cursor(command) => {
                match &mut self.library.focus {
                    FocusedPane::FeedsList => {
                        self.library.feeds.scroll(command);
                        self.update_current_feed(ctx);
                    }
                    FocusedPane::EpisodesList => self.library.episodes.scroll(command),
                    FocusedPane::Search => {
                        if let SearchState::Loaded(list) = &mut self.library.search {
                            list.scroll(command);
                        }
                    }
                    FocusedPane::ErrorsLog => self.log_history.scroll(command),
                }
                self.invalidate_later(ctx);
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
                    log::info!(target: "key_mapping", "Key mapping redefined");
                }
            }
            Command::Unmap(key, state) => {
                if !self.key_mapping.unmap(key, state) {
                    log::info!(target: "key_mapping", "Key mapping is not defined");
                }
            }
            Command::Theme(command) => {
                if let Err(_error) = self.theme.handle_command(command, &self.app_env) {
                    // self.handle_error(error, ctx);
                } else {
                    self.invalidate(ctx);
                }
            }
            Command::Exec(path) => {
                let file_path = self.app_env.resolve_config(&path);
                let mut reader = match CommandReader::open(file_path) {
                    Ok(reader) => reader,
                    Err(_error) => {
                        // self.handle_error(error, ctx);
                        return;
                    }
                };

                loop {
                    match reader.read(()) {
                        Ok(None) => break,
                        Ok(Some(command)) => {
                            self.handle_command(command, ctx);
                            // if self.status.data().has_errors() {
                            //     return;
                            // }
                        }
                        Err(_error) => {
                            // self.handle_error(error, ctx);
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
                    // self.status
                    //     .update_data::<selection::DoNotUpdate, _>(|data| {
                    //         if let Some(status::ErrorType::Playback) =
                    //             data.display_status().and_then(Status::error_type)
                    //         {
                    //             data.clear_display();
                    //         }
                    //     });
                    episode_id
                } else {
                    return;
                };
                self.invalidate_later(ctx);

                let future = wrap_future(
                    self.library_actor
                        .send(EpisodePlaybackDataRequest(episode_id)),
                )
                .map(move |result, actor: &mut UI, ctx| {
                    if let Some(playback_data) = actor.handle_response_error(result) {
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
            Command::Finish => {
                if let Some(playing) = &self.library.playing_episode {
                    self.player_actor.do_send(PlaybackCommand::Stop);
                    self.status_writer_actor
                        .do_send(StatusWriterCommand::set_finished(playing.id));
                    self.library
                        .episodes
                        .update_data::<selection::DoNotUpdate, _>(|data| {
                            let episode = data
                                .find(|item| item.id == playing.id)
                                .and_then(|index| data.item_at_mut(index));
                            if let Some(episode) = episode {
                                episode.status = EpisodeSummaryStatus::Finished;
                            }
                        });
                }
            }
            Command::Volume(command) => self.player_actor.do_send(command),
            Command::AddFeed(source) => self
                .library_actor
                .do_send(FeedUpdateRequest::AddFeed(NewFeedMetadata::new(source))),
            Command::DeleteFeed => {
                if let Some(FeedView::Feed(selected_feed)) = self.library.feeds.selection() {
                    self.library_actor
                        .do_send(FeedUpdateRequest::DeleteFeed(selected_feed.id));
                }
            }
            Command::Update { current_only } => {
                let query = if current_only {
                    self.selected_feed
                        .and_then(|feed| feed.as_feed().cloned())
                        .map(UpdateQuery::Single)
                } else {
                    Some(UpdateQuery::All)
                };
                if let Some(query) = query {
                    self.library_actor.do_send(FeedUpdateRequest::Update(query));
                }
            }
            Command::SetOption(options_update) => {
                let affects_episodes_list = options_update.affects_episodes_list();
                self.options.update(options_update);
                if affects_episodes_list {
                    self.refresh_episodes(ctx, false);
                }
                self.invalidate(ctx);
            }
            Command::SetFeedEnabled(enabled) => {
                if let Some(FeedView::Feed(selected_feed)) = self.selected_feed {
                    self.library_actor
                        .do_send(FeedUpdateRequest::SetFeedEnabled(selected_feed, enabled));
                }
            }
            Command::Mark {
                status,
                update_all,
                condition,
            } => {
                if update_all {
                    if let Some(feed) = self.selected_feed {
                        self.library
                            .episodes
                            .update_data::<selection::DoNotUpdate, _>(|data| {
                                for episode in data.iter_mut() {
                                    if condition.is_none() || condition == Some(episode.status) {
                                        episode.status = (&status).into();
                                    }
                                }
                            });

                        let mut query = EpisodesQuery::from_feed_view(feed);
                        if let Some(condition) = condition {
                            query = query.status(condition);
                        }
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
                                if condition.is_none() || condition == Some(selected.status) {
                                    selected.status = (&status).into();
                                }
                            }
                        });

                    let mut query = EpisodesQuery::default().id(selected_id);
                    if let Some(condition) = condition {
                        query = query.status(condition);
                    }
                    self.status_writer_actor
                        .do_send(StatusWriterCommand::Set(query, status));
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
                        self.invalidate_later(ctx);
                    }
                }
            }
            Command::Refresh => {
                self.library
                    .episodes
                    .update_data::<selection::Reset, _>(|data| {
                        data.clear_provider();
                        data.clear();
                    });
                self.selected_feed = None;
                self.library.episodes_list_metadata = None;
                self.load_feeds(ctx);
                self.invalidate(ctx);
            }
            Command::SetEpisodeHidden(hidden) => {
                let query = self
                    .library
                    .episodes
                    .selection()
                    .map(|episode| EpisodesQuery::default().id(episode.id));

                if let Some(query) = query {
                    self.library_actor
                        .do_send(FeedUpdateRequest::SetHidden(query, hidden));
                    self.refresh_episodes(ctx, false);
                }
            }
            Command::Reverse => {
                match self
                    .selected_feed
                    .and_then(|feed_view| feed_view.as_feed().cloned())
                {
                    Some(feed_id) => {
                        self.library_actor
                            .do_send(FeedUpdateRequest::ReverseFeedOrder(feed_id));
                        self.refresh_episodes(ctx, true);
                    }
                    None => {
                        log::warn!("Only individual podcast's orders can be reversed");
                    }
                }
            }
            Command::OpenLink(LinkType::Feed) => {
                if let Some(FeedView::Feed(feed_id)) = self.selected_feed {
                    ctx.spawn(
                        wrap_future(
                            self.library_actor
                                .send(hedgehog_library::FeedRequest(feed_id)),
                        )
                        .map(move |result, actor: &mut UI, _ctx| {
                            if let Some(Some(Feed {
                                link: Some(link), ..
                            })) = actor.handle_response_error(result)
                            {
                                actor.open_browser(&link);
                            }
                        }),
                    );
                }
            }
            Command::OpenLink(LinkType::Episode) => {
                if let Some(episode_id) =
                    self.library.episodes.selection().map(|episode| episode.id)
                {
                    ctx.spawn(
                        wrap_future(
                            self.library_actor
                                .send(hedgehog_library::EpisodeRequest(episode_id)),
                        )
                        .map(move |result, actor: &mut UI, _ctx| {
                            if let Some(Some(Episode {
                                link: Some(link), ..
                            })) = actor.handle_response_error(result)
                            {
                                actor.open_browser(&link);
                            }
                        }),
                    );
                }
            }
        }
    }

    fn open_browser(&mut self, url: &str) {
        log::info!("Opening '{}'", url);
        if let Err(error) = webbrowser::open(url) {
            log::error!("{}", error);
        }
    }

    fn init_rc(&mut self, ctx: &mut <UI as Actor>::Context) {
        for path in self.app_env.resolve_rc("rc") {
            self.handle_command(Command::Exec(path.to_path_buf()), ctx);
            // if self.status.data().has_errors() {
            //     break;
            // }
        }

        self.library_actor
            .do_send(FeedUpdateRequest::Update(if self.options.update_on_start {
                UpdateQuery::All
            } else {
                UpdateQuery::Pending
            }));
    }

    fn refresh_episodes(&mut self, ctx: &mut <UI as Actor>::Context, replace_current: bool) {
        let feed_id = match self.selected_feed {
            Some(feed_id) => feed_id,
            None => return,
        };
        self.library
            .episodes
            .update_data::<selection::Keep, _>(|data| {
                // To prevent updates for the old data
                data.clear_provider();
                if replace_current {
                    data.clear();
                }
            });
        if replace_current {
            self.library.episodes_list_metadata = None;
        }

        let query = EpisodesQuery::from_feed_view(feed_id).with_hidden(self.options.hidden);
        let new_provider = EpisodesListProvider {
            query: query.clone(),
            actor: ctx.address(),
        };
        let future = wrap_future(
            self.library_actor
                .send(EpisodesListMetadataRequest(query.clone())),
        )
        .then(|result, actor: &mut UI, _ctx| {
            let result = actor.handle_response_error(result).map(|metadata| {
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
                        let query = query.clone().reversed_order(metadata.reversed_order);
                        let episodes = library_actor
                            .send(EpisodeSummariesRequest::new(query, range.clone()))
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
                        episodes.update_data::<selection::FindPrevious<selection::Keep>, _>($fn);
                    }
                }};
            }
            if let Some((metadata, episodes)) = result {
                let items_count = metadata.items_count;
                actor.library.episodes_list_metadata = Some(metadata);
                match episodes {
                    Some((range, episodes)) => {
                        if let Some(episodes) = actor.handle_response_error(episodes) {
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
            self.library.episodes_list_metadata = None;
        }
        self.invalidate_later(ctx);
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

    fn clear_log_display(&mut self, ctx: &mut <UI as Actor>::Context) {
        self.log_history
            .update_data::<selection::Reset, _>(LogHistory::clear_display);
        if let Some(handle) = self.log_display_clear_request.take() {
            ctx.cancel_future(handle);
        }
    }

    fn load_feeds(&mut self, ctx: &mut <UI as Actor>::Context) {
        ctx.spawn(
            wrap_future(self.library_actor.send(FeedSummariesRequest)).map(
                move |data, actor: &mut UI, ctx| {
                    if let Some(data) = actor.handle_response_error(data) {
                        actor
                            .library
                            .feeds
                            .update_data::<selection::FindPrevious, _>(|current_feeds| {
                                let mut feeds = Vec::with_capacity(data.len() + 2);
                                feeds.push(FeedView::All);
                                feeds.push(FeedView::New);
                                feeds.extend(data.into_iter().map(FeedView::Feed));
                                *current_feeds = feeds;
                            });
                        actor.update_current_feed(ctx);
                        actor.library.feeds_loaded = true;
                        actor.invalidate(ctx);
                    }
                },
            ),
        );
    }
}

impl Actor for UI {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.load_feeds(ctx);

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

        ctx.add_stream(event::EventStream::new());

        self.init_rc(ctx);
        if let Err(error) = self.commands_history.load_file(self.app_env.history_path()) {
            log::error!(target: "commands_history", "{}", error);
        }

        self.invalidate(ctx);
    }
}

impl StreamHandler<crossterm::Result<event::Event>> for UI {
    fn handle(&mut self, item: crossterm::Result<event::Event>, ctx: &mut Self::Context) {
        let event = match item {
            Ok(Event::Resize(_, height)) => {
                let lib_height = height.saturating_sub(2) as usize;
                self.library.set_window_size(lib_height);
                self.log_history.set_window_size(lib_height / 3);
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
                    self.clear_log_display(ctx);
                    self.command = Some(CommandState::default());
                    self.invalidate(ctx);
                }
                event::Event::Key(key_event) => {
                    let command = self
                        .key_mapping
                        .get(key_event.into(), Some(self.library.focus));
                    if let Some(command) = command.cloned() {
                        self.handle_command(command, ctx);
                    }
                }
                event::Event::Mouse(event) => {
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
                            let offset = match event.kind == MouseEventKind::ScrollUp {
                                true => ScrollAction::ScrollBy(-3),
                                false => ScrollAction::ScrollBy(3),
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
                                        list.scroll(offset.with_amount_abs(1));
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
                                            SeekOffset(seek_direction, Duration::from_secs(1)),
                                        )),
                                        ctx,
                                    );
                                }
                                MouseHitResult::CommandEntry(_) => (),
                            }
                            self.invalidate_later(ctx);
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
                                    let item_clicked =
                                        self.library.episodes.has_item_at_window_row(row);
                                    if item_clicked {
                                        self.library
                                            .episodes
                                            .scroll(ScrollAction::MoveToVisible(row));
                                        if is_double {
                                            self.handle_command(Command::PlayCurrent, ctx);
                                        }
                                    }
                                }
                                MouseHitResult::SearchRow(row) => {
                                    self.library.focus = FocusedPane::Search;
                                    if let SearchState::Loaded(ref mut list) = self.library.search {
                                        if list.has_item_at_window_row(row) {
                                            list.scroll(ScrollAction::MoveToVisible(row));
                                            if is_double {
                                                self.handle_command(Command::SearchAdd, ctx);
                                            }
                                        }
                                    }
                                }
                                MouseHitResult::Player => {
                                    self.handle_command(
                                        Command::Playback(PlaybackCommand::TogglePause),
                                        ctx,
                                    );
                                }
                                MouseHitResult::CommandEntry(_) => {
                                    self.clear_log_display(ctx);
                                    self.command = Some(CommandState::default());
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
            Some(ref mut command_state) => match event {
                event::Event::Mouse(event::MouseEvent {
                    kind:
                        event::MouseEventKind::Down(event::MouseButton::Left)
                        | event::MouseEventKind::Drag(event::MouseButton::Left)
                        | event::MouseEventKind::Up(event::MouseButton::Left),
                    row,
                    column,
                    ..
                }) => {
                    if let Some(MouseHitResult::CommandEntry(offset)) =
                        self.layout.hit_test_at(row, column)
                    {
                        command_state.set_display_position(offset.saturating_sub(1) as u16);
                        self.invalidate(ctx);
                    }
                }
                event => match command_state.handle_event(event, &self.commands_history) {
                    CommandActionResult::None => (),
                    CommandActionResult::Update => self.invalidate_later(ctx),
                    CommandActionResult::Clear => {
                        self.command = None;
                        self.invalidate(ctx);
                    }
                    CommandActionResult::Complete => {
                        let command_str =
                            command_state.as_str_before_cursor(&self.commands_history);
                        let completion: Vec<_> = cmdparse::complete::<_, Command>(command_str, ())
                            .into_iter()
                            .collect();
                        command_state.set_completions(completion);
                        self.invalidate(ctx);
                    }
                    CommandActionResult::Submit => {
                        let command_str = command_state.as_str(&self.commands_history).to_string();
                        if let Err(error) = self.commands_history.push(&command_str) {
                            log::error!(target: "commands_history", "{}", error);
                        }
                        self.command = None;
                        match cmdparse::parse::<_, Option<Command>>(&command_str, ()) {
                            Ok(Some(command)) => self.handle_command(command, ctx),
                            Ok(None) => {}
                            Err(error) => log::error!(target: "command", "{}", error),
                        }
                        self.invalidate(ctx);
                    }
                },
            },
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
    fn handle_response_error<T>(&mut self, data: LibraryQueryResult<T>) -> Option<T> {
        match data {
            Err(err) => log::error!(target: "actix", "{}", err),
            Ok(Err(err)) => log::error!("{}", err),
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
                Box::pin(wrap_future(self.library_actor.send(request)).map(
                    move |data, actor: &mut UI, ctx| {
                        if let Some(episodes) = actor.handle_response_error(data) {
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
            PlayerNotification::VolumeChanged(volume) => match volume {
                None => log::info!(target: "volume", "Playback muted"),
                Some(volume) => {
                    log::info!(target: "volume", "Volume: {:.0}%", volume.cubic() * 100.0);
                }
            },
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

    fn handle(&mut self, _msg: PlayerErrorNotification, ctx: &mut Self::Context) -> Self::Result {
        // TODO: self.handle_error(msg.0, ctx);
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
                    || self.selected_feed.map(|view| view.as_feed().is_none()) == Some(true)
                {
                    self.refresh_episodes(ctx, false);
                }
            }
            FeedUpdateNotification::Error(_error) => (), /* TODO: self.handle_error(error, ctx) */
            FeedUpdateNotification::FeedAdded(feed) => {
                self.library
                    .feeds
                    .update_data::<selection::Keep, _>(|feeds| feeds.push(FeedView::Feed(feed)));
                self.update_current_feed(ctx);
            }
            FeedUpdateNotification::DuplicateFeed => ()/* TODO: self.set_status(
                CustomStatus::new("This podcast has already been added")
                    .set_severity(Severity::Warning)
                    .set_ttl(status::TTL_SHORT)
                    .into(),
                ctx,
            )*/,
            FeedUpdateNotification::FeedDeleted(feed_id) => {
                self.library
                    .feeds
                    .update_data::<selection::FindPrevious<selection::Keep>, _>(|feeds| {
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
            FeedUpdateNotification::NewCountUpdated(new_count) => {
                if self.selected_feed == Some(FeedView::New) {
                    self.refresh_episodes(ctx, false);
                }
                self.library
                    .feeds
                    .update_data::<selection::DoNotUpdate, _>(|feeds| {
                        for feed in feeds {
                            if let Some(feed) = feed.as_mut() {
                                if let Some(new) = new_count.get(&feed.id) {
                                    feed.new_count = *new;
                                }
                            }
                        }
                    });
            }
        }
        self.invalidate(ctx);
    }
}

impl Handler<LogEntry> for UI {
    type Result = ();

    fn handle(&mut self, entry: LogEntry, ctx: &mut Self::Context) -> Self::Result {
        if let Some(handle) = self.log_display_clear_request.take() {
            ctx.cancel_future(handle);
        }
        if let Some(duration) = entry.display_ttl() {
            self.log_display_clear_request = Some(ctx.spawn(wrap_future(sleep(duration)).map(
                |_, actor: &mut UI, ctx| {
                    actor.log_display_clear_request = None;
                    actor
                        .log_history
                        .update_data::<selection::DoNotUpdate, _>(LogHistory::clear_display);
                    actor.invalidate(ctx);
                },
            )));
        }
        self.log_history
            .update_data::<selection::Keep, _>(|log| log.push(entry));
        self.invalidate(ctx);
    }
}
