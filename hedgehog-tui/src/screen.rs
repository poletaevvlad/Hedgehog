use crate::dataview::{
    DataProvider, ListDataRequest, PaginatedDataMessage, PaginatedDataRequest, Version, Versioned,
};
use crate::events::key;
use crate::history::CommandsHistory;
use crate::theming;
use crate::view_model::{Command, FocusedPane, ViewModel};
use crate::widgets::command::{CommandActionResult, CommandEditor, CommandState};
use crate::widgets::library_rows::{EpisodesListRowRenderer, FeedsListRowRenderer};
use crate::widgets::list::List;
use crate::widgets::player_state::PlayerState;
use actix::prelude::*;
use crossterm::event::Event;
use hedgehog_library::datasource::QueryError;
use hedgehog_library::model::{EpisodeSummary, FeedSummary};
use hedgehog_library::{
    EpisodeSummariesQuery, FeedSummariesQuery, Library, PagedQueryRequest, QueryRequest,
    SizeRequest,
};
use hedgehog_player::{Player, PlayerNotification};
use tui::backend::CrosstermBackend;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::text::Span;
use tui::widgets::{Block, Borders, Paragraph};
use tui::Terminal;

pub(crate) struct UI {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    command: Option<CommandState>,
    commands_history: CommandsHistory,
    library: Addr<Library>,
    player: Addr<Player>,
    view_model: ViewModel,
}

impl UI {
    pub(crate) fn new(
        size: (u16, u16),
        terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
        library: Addr<Library>,
        player: Addr<Player>,
    ) -> Self {
        UI {
            terminal,
            command: None,
            commands_history: CommandsHistory::new(),
            library,
            player,
            view_model: ViewModel::new(size),
        }
    }

    fn render(&mut self) {
        let command = &mut self.command;
        let history = &self.commands_history;
        let episodes_list = &self.view_model.episodes_list;
        let view_model = &self.view_model;

        let draw = |f: &mut tui::Frame<CrosstermBackend<std::io::Stdout>>| {
            let area = f.size();
            let library_rect = Rect::new(0, 0, area.width, area.height - 2);

            let layout = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(24), Constraint::Percentage(75)].as_ref())
                .split(library_rect);

            let feeds_border = Block::default()
                .borders(Borders::RIGHT)
                .border_style(view_model.theme.get(theming::List::Divider));
            let feeds_area = feeds_border.inner(layout[0]);
            f.render_widget(feeds_border, layout[0]);

            if let Some(iter) = view_model.feeds_list.iter() {
                f.render_widget(
                    List::new(
                        FeedsListRowRenderer::new(
                            &view_model.theme,
                            view_model.focus == FocusedPane::FeedsList,
                        ),
                        iter,
                    ),
                    feeds_area,
                );
            }
            if let Some(iter) = episodes_list.iter() {
                f.render_widget(
                    List::new(
                        EpisodesListRowRenderer::new(
                            &view_model.theme,
                            view_model.focus == FocusedPane::EpisodesList,
                        ),
                        iter,
                    ),
                    layout[1],
                );
            }

            let player_widget = PlayerState::new(&view_model.playback_state, &view_model.theme);
            let player_rect = Rect::new(0, area.height - 2, area.width, 1);
            f.render_widget(player_widget, player_rect);

            let status_rect = Rect::new(0, area.height - 1, area.width, 1);
            if let Some(ref mut command_state) = command {
                let style = view_model.theme.get(theming::StatusBar::Command);
                let prompt_style = view_model.theme.get(theming::StatusBar::CommandPrompt);
                CommandEditor::new(command_state)
                    .prefix(Span::styled(":", prompt_style))
                    .style(style)
                    .render(f, status_rect, history);
            } else if let Some(status) = &view_model.status {
                let theme_selector = theming::StatusBar::Status(Some(status.severity()));
                let style = view_model.theme.get(theme_selector);
                f.render_widget(Paragraph::new(status.to_string()).style(style), status_rect);
            } else {
                f.render_widget(
                    Block::default().style(view_model.theme.get(theming::StatusBar::Empty)),
                    status_rect,
                );
            }
        };
        self.terminal.draw(draw).unwrap();
    }

    fn handle_error<T, E>(&mut self, result: Result<T, E>) -> Option<T>
    where
        E: std::error::Error,
    {
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                self.view_model.error(error);
                None
            }
        }
    }
}

impl Actor for UI {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.add_stream(crossterm::event::EventStream::new());
        self.view_model
            .episodes_list
            .set_provider(EpisodesListProvider {
                query: None,
                actor: ctx.address(),
            });
        self.view_model.feeds_list.set_provider(FeedsListProvider {
            query: FeedSummariesQuery,
            actor: ctx.address(),
        });
        self.view_model.init_rc();
        self.player
            .do_send(hedgehog_player::ActorCommand::Subscribe(
                ctx.address().recipient(),
            ));
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
                    match self.view_model.key_mapping.get(&key_event.into()) {
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
    pub(crate) query: Option<EpisodeSummariesQuery>,
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
    query: FeedSummariesQuery,
    actor: Addr<UI>,
}

impl DataProvider for FeedsListProvider {
    type Request = ListDataRequest;

    fn request(&self, request: Versioned<Self::Request>) {
        self.actor
            .do_send(DataFetchingRequest::Feeds(self.query.clone(), request));
    }
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
enum DataFetchingRequest {
    Episodes(EpisodeSummariesQuery, Versioned<PaginatedDataRequest>),
    Feeds(FeedSummariesQuery, Versioned<ListDataRequest>),
}

type LibraryQueryResult<T> = Result<Result<T, QueryError>, MailboxError>;

impl UI {
    fn handle_library_response<T>(
        &mut self,
        data: LibraryQueryResult<T>,
        handler: impl FnOnce(&mut Self, T) -> bool,
    ) {
        let data = self.handle_error(data).and_then(|r| self.handle_error(r));
        let should_render = match data {
            Some(data) => handler(self, data),
            None => true,
        };
        if should_render {
            self.render();
        }
    }

    fn handle_episode_size_response(&mut self, version: Version, size: LibraryQueryResult<usize>) {
        self.handle_library_response(size, move |actor, size| {
            actor.view_model.set_episodes_list_data(
                Versioned::new(PaginatedDataMessage::size(size)).with_version(version),
            )
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
                    PaginatedDataRequest::Size => {
                        Box::pin(self.library.send(SizeRequest(query)).into_actor(self).map(
                            move |size, actor, _ctx| {
                                actor.handle_episode_size_response(version, size)
                            },
                        ))
                    }
                    PaginatedDataRequest::Page { index, range } => {
                        let request = PagedQueryRequest {
                            data: query,
                            offset: range.start,
                            count: range.len(),
                        };
                        Box::pin(self.library.send(request).into_actor(self).map(
                            move |data, actor, _ctx| {
                                actor.handle_episode_data_response(version, data, index)
                            },
                        ))
                    }
                }
            }
            DataFetchingRequest::Feeds(query, request) => {
                Box::pin(self.library.send(QueryRequest(query)).into_actor(self).map(
                    move |data, actor, _ctx| {
                        actor.handle_feeds_data_response(request.version(), data)
                    },
                ))
            }
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
