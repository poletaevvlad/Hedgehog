use crate::dataview::{
    DataProvider, InteractiveList, ListData, ListDataRequest, PaginatedData, PaginatedDataMessage,
    PaginatedDataRequest, Version, Versioned,
};
use crate::events::key;
use crate::history::CommandsHistory;
use crate::status::{Severity, Status};
use crate::view_model::{ActionDelegate, Command, FocusedPane, ViewModel};
use crate::widgets::command::{CommandActionResult, CommandEditor, CommandState};
use crate::widgets::library::LibraryWidget;
use crate::widgets::player_state::PlayerState;
use crate::widgets::split_bottom;
use crate::widgets::status::StatusView;
use actix::prelude::*;
use crossterm::event::Event;
use crossterm::{terminal, QueueableCommand};
use hedgehog_library::datasource::QueryError;
use hedgehog_library::model::{
    EpisodeId, EpisodeSummary, EpisodesListMetadata, FeedId, FeedSummary,
};
use hedgehog_library::status_writer::StatusWriter;
use hedgehog_library::{
    EpisodePlaybackDataRequest, EpisodeSummariesRequest, EpisodesListMetadataRequest,
    EpisodesQuery, FeedSummariesRequest, FeedUpdateNotification, Library,
};
use hedgehog_player::{Player, PlayerErrorNotification, PlayerNotification};
use std::collections::HashSet;
use std::io::{stdout, Write};
use tui::backend::CrosstermBackend;
use tui::Terminal;

pub(crate) struct LibraryViewModel {
    pub(crate) feeds: InteractiveList<ListData<FeedSummary>, FeedsListProvider>,
    pub(crate) episodes: InteractiveList<PaginatedData<EpisodeSummary>, EpisodesListProvider>,
    pub(crate) episodes_list_metadata: Option<EpisodesListMetadata>,
    pub(crate) focus: FocusedPane,
    pub(crate) updating_feeds: HashSet<FeedId>,
    pub(crate) playing_episode: Option<EpisodeSummary>,
}

pub(crate) struct UI {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    command: Option<CommandState>,
    commands_history: CommandsHistory,
    library: Addr<Library>,
    player: Addr<Player>,
    view_model: ViewModel<ActorActionDelegate>,
}

impl UI {
    pub(crate) fn new(
        size: (u16, u16),
        terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
        library: Addr<Library>,
        player: Addr<Player>,
        status_writer: Addr<StatusWriter>,
    ) -> Self {
        UI {
            terminal,
            command: None,
            commands_history: CommandsHistory::new(),
            library: library.clone(),
            player: player.clone(),
            view_model: ViewModel::new(
                size,
                ActorActionDelegate {
                    ui: None,
                    player,
                    library,
                    status_writer,
                },
            ),
        }
    }

    fn render(&mut self) {
        let draw = |f: &mut tui::Frame<CrosstermBackend<std::io::Stdout>>| {
            let area = f.size();
            let (area, status_area) = split_bottom(area, 1);
            let (area, player_area) = split_bottom(area, 1);

            let library_widget = LibraryWidget::new(
                &self.view_model.library,
                &self.view_model.options,
                &self.view_model.theme,
            );
            f.render_widget(library_widget, area);

            let player_widget = PlayerState::new(
                &self.view_model.playback_state,
                &self.view_model.theme,
                self.view_model.library.playing_episode.as_ref(),
            );
            f.render_widget(player_widget, player_area);

            if let Some(ref mut command_state) = self.command {
                CommandEditor::new(command_state)
                    .prefix(":")
                    .theme(&self.view_model.theme)
                    .render(f, status_area, &self.commands_history);
            } else {
                let status =
                    StatusView::new(self.view_model.status.as_ref(), &self.view_model.theme);
                f.render_widget(status, status_area);
            }
        };
        self.terminal.draw(draw).unwrap();

        let playing_episode = self.view_model.library.playing_episode.as_ref();
        let episode_title = playing_episode.and_then(|episode| episode.title.as_deref());
        let title = match episode_title {
            Some(title) => format!("{} | hedgehog", title),
            None => "hedgehog".to_string(),
        };
        let mut stdout = stdout();
        stdout.queue(terminal::SetTitle(title)).unwrap();
        stdout.flush().unwrap();
    }
}

impl Actor for UI {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.view_model.action_delegate.ui = Some(ctx.address());
        self.view_model
            .library
            .episodes
            .set_provider(EpisodesListProvider {
                query: None,
                actor: ctx.address(),
            });
        self.view_model
            .library
            .feeds
            .set_provider(FeedsListProvider {
                actor: ctx.address(),
            });
        self.view_model.init_rc();
        self.player
            .do_send(hedgehog_player::ActorCommand::Subscribe(
                ctx.address().recipient(),
            ));
        self.player
            .do_send(hedgehog_player::ActorCommand::SubscribeErrors(
                ctx.address().recipient(),
            ));

        ctx.add_stream(crossterm::event::EventStream::new());
        self.render();
    }
}

impl StreamHandler<crossterm::Result<crossterm::event::Event>> for UI {
    fn handle(
        &mut self,
        item: crossterm::Result<crossterm::event::Event>,
        _ctx: &mut Self::Context,
    ) {
        let event = match item {
            Ok(Event::Resize(width, height)) => {
                self.view_model.set_size(width, height);
                self.render();
                return;
            }
            Ok(event) => event,
            Err(_) => {
                System::current().stop();
                return;
            }
        };

        let should_render = match self.command {
            None => match event {
                key!('c', CONTROL) => self.view_model.handle_command_interactive(Command::Quit),
                key!(':') => {
                    self.view_model.clear_status();
                    self.command = Some(CommandState::default());
                    true
                }
                crossterm::event::Event::Key(key_event) => {
                    match self
                        .view_model
                        .key_mapping
                        .get(key_event.into(), Some(self.view_model.library.focus))
                    {
                        Some(command) => {
                            let command = command.clone();
                            self.view_model.handle_command_interactive(command)
                        }
                        None => false,
                    }
                }
                _ => false,
            },
            Some(ref mut command_state) => {
                match command_state.handle_event(event, &self.commands_history) {
                    CommandActionResult::None => false,
                    CommandActionResult::Update => true,
                    CommandActionResult::Clear => {
                        self.command = None;
                        true
                    }
                    CommandActionResult::Submit => {
                        let command_str = command_state.as_str(&self.commands_history).to_string();
                        self.commands_history.push(&command_str);
                        self.command = None;
                        self.view_model.handle_command_str(command_str.as_str());
                        true
                    }
                }
            }
        };
        if should_render {
            self.render();
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
    fn handle_library_response<T>(
        &mut self,
        data: LibraryQueryResult<T>,
        handler: impl FnOnce(&mut Self, T) -> bool,
    ) {
        let should_render = match data {
            Err(err) => {
                self.view_model.error(err);
                true
            }
            Ok(Err(err)) => {
                self.view_model.error(err);
                true
            }
            Ok(Ok(data)) => handler(self, data),
        };
        if should_render {
            self.render();
        }
    }

    fn handle_episode_size_response(
        &mut self,
        version: Version,
        metadata: LibraryQueryResult<EpisodesListMetadata>,
    ) {
        self.handle_library_response(metadata, move |actor, metadata| {
            let should_render = actor.view_model.set_episodes_list_data(
                Versioned::new(PaginatedDataMessage::size(metadata.items_count))
                    .with_version(version),
            );
            actor.view_model.library.episodes_list_metadata = Some(metadata);
            should_render
        });
    }

    fn handle_episode_data_response(
        &mut self,
        version: Version,
        data: LibraryQueryResult<Vec<EpisodeSummary>>,
        page_index: usize,
    ) {
        self.handle_library_response(data, move |actor, data| {
            let message = PaginatedDataMessage::page(page_index, data);
            actor
                .view_model
                .set_episodes_list_data(Versioned::new(message).with_version(version))
        });
    }

    fn handle_feeds_data_response(
        &mut self,
        version: Version,
        data: LibraryQueryResult<Vec<FeedSummary>>,
    ) {
        self.handle_library_response(data, move |actor, data| {
            actor
                .view_model
                .set_feeds_list_data(Versioned::new(data).with_version(version))
        });
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
                        self.library
                            .send(EpisodesListMetadataRequest(query))
                            .into_actor(self)
                            .map(move |metadata, actor, _ctx| {
                                actor.handle_episode_size_response(version, metadata);
                            }),
                    ),
                    PaginatedDataRequest::Page(page) => {
                        let page_index = page.index;
                        let request = EpisodeSummariesRequest::new(query, page);
                        Box::pin(self.library.send(request).into_actor(self).map(
                            move |data, actor, _ctx| {
                                actor.handle_episode_data_response(version, data, page_index);
                            },
                        ))
                    }
                }
            }
            DataFetchingRequest::Feeds(request) => Box::pin(
                self.library
                    .send(FeedSummariesRequest)
                    .into_actor(self)
                    .map(move |data, actor, _ctx| {
                        actor.handle_feeds_data_response(request.version(), data);
                    }),
            ),
        }
    }
}

impl Handler<PlayerNotification> for UI {
    type Result = ();

    fn handle(&mut self, msg: PlayerNotification, _ctx: &mut Self::Context) -> Self::Result {
        self.view_model.handle_player_notification(msg);
        self.render();
    }
}

impl Handler<PlayerErrorNotification> for UI {
    type Result = ();

    fn handle(&mut self, msg: PlayerErrorNotification, _ctx: &mut Self::Context) -> Self::Result {
        self.view_model.status = Some(Status::new_custom(msg.0.to_string(), Severity::Error));
    }
}

impl Handler<FeedUpdateNotification> for UI {
    type Result = ();

    fn handle(&mut self, msg: FeedUpdateNotification, _ctx: &mut Self::Context) -> Self::Result {
        self.view_model.handle_update_notification(msg);
        self.render();
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct StartPlaybackRequest(EpisodeId);

impl Handler<StartPlaybackRequest> for UI {
    type Result = ResponseActFuture<Self, ()>;

    fn handle(&mut self, msg: StartPlaybackRequest, _ctx: &mut Self::Context) -> Self::Result {
        let future = self
            .library
            .send(EpisodePlaybackDataRequest(msg.0))
            .into_actor(self)
            .map(|result, actor, _ctx| match result {
                Ok(Ok(playback_data)) => {
                    actor.player.do_send(hedgehog_player::PlaybackCommand::Play(
                        playback_data.media_url,
                        playback_data.position,
                    ));
                }
                Ok(Err(error)) => actor.view_model.error(error),
                Err(error) => actor.view_model.error(error),
            });
        Box::pin(future)
    }
}

struct ActorActionDelegate {
    ui: Option<Addr<UI>>,
    player: Addr<Player>,
    library: Addr<Library>,
    status_writer: Addr<StatusWriter>,
}

impl ActionDelegate for ActorActionDelegate {
    fn start_playback(&self, episode_id: EpisodeId) {
        self.ui
            .as_ref()
            .expect("ui is not initialized")
            .do_send(StartPlaybackRequest(episode_id));
    }

    fn send_volume_command(&self, command: hedgehog_player::volume::VolumeCommand) {
        self.player.do_send(command);
    }

    fn send_playback_command(&self, command: hedgehog_player::PlaybackCommand) {
        self.player.do_send(command);
    }

    fn send_feed_update_request(&self, command: hedgehog_library::FeedUpdateRequest) {
        self.library.do_send(command);
    }

    fn send_status_write_request(
        &self,
        command: hedgehog_library::status_writer::StatusWriterCommand,
    ) {
        self.status_writer.do_send(command);
    }
}
