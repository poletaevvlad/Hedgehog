use crate::dataview::{DataProvider, PaginatedDataMessage, PaginatedDataRequest, Versioned};
use crate::events::key;
use crate::history::CommandsHistory;
use crate::status::Severity;
use crate::view_model::{Command, ViewModel};
use crate::widgets::command::{CommandActionResult, CommandEditor, CommandState};
use crate::widgets::list::{List, ListItemRenderingDelegate};
use actix::prelude::*;
use crossterm::event::Event;
use hedgehog_library::{EpisodeSummariesQuery, Library, QueryRequest, SizeRequest};
use tui::backend::CrosstermBackend;
use tui::layout::Rect;
use tui::style::{Color, Style};
use tui::text::Span;
use tui::widgets::{Paragraph, Widget};
use tui::Terminal;

pub(crate) struct UI {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    command: Option<CommandState>,
    commands_history: CommandsHistory,
    library: Addr<Library>,
    view_model: ViewModel,
}

impl UI {
    pub(crate) fn new(
        size: (u16, u16),
        terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
        library: Addr<Library>,
    ) -> Self {
        UI {
            terminal,
            command: None,
            commands_history: CommandsHistory::new(),
            library,
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
            let library_rect = Rect::new(0, 0, area.width, area.height - 1);
            if let Some(iter) = episodes_list.iter() {
                f.render_widget(List::new(EpisodesListRowRenderer, iter), library_rect);
            }

            let status_rect = Rect::new(0, area.height - 1, area.width, 1);
            if let Some(ref mut command_state) = command {
                CommandEditor::new(command_state)
                    .prefix(Span::raw(":"))
                    .render(f, status_rect, history);
            } else if let Some(status) = &view_model.status {
                let color = match status.severity() {
                    Severity::Error => Color::Red,
                    Severity::Warning => Color::Yellow,
                    Severity::Information => Color::LightBlue,
                };
                f.render_widget(
                    Paragraph::new(status.to_string()).style(Style::default().fg(color)),
                    status_rect,
                );
            }
        };
        self.terminal.draw(draw).unwrap();
    }
}

impl Actor for UI {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.add_stream(crossterm::event::EventStream::new());
        self.view_model
            .episodes_list
            .set_provider(EpisodesListProvider {
                query: EpisodeSummariesQuery { feed_id: None },
                actor: ctx.address(),
            });
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
                key!('c', CONTROL) => {
                    self.view_model.handle_command(Command::Quit);
                    false
                }
                key!(':') => {
                    self.view_model.clear_status();
                    self.command = Some(CommandState::default());
                    true
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
    query: EpisodeSummariesQuery,
    actor: Addr<UI>,
}

impl DataProvider for EpisodesListProvider {
    type Request = PaginatedDataRequest;

    fn request(&self, request: crate::dataview::Versioned<Self::Request>) {
        self.actor
            .do_send(DataFetchingRequest::Episodes(self.query.clone(), request));
    }
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
enum DataFetchingRequest {
    Episodes(EpisodeSummariesQuery, Versioned<PaginatedDataRequest>),
}

impl Handler<DataFetchingRequest> for UI {
    type Result = ResponseActFuture<Self, ()>;

    fn handle(&mut self, msg: DataFetchingRequest, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            DataFetchingRequest::Episodes(query, request) => {
                let version = request.version();
                match request.unwrap() {
                    PaginatedDataRequest::Size => {
                        Box::pin(self.library.send(SizeRequest(query)).into_actor(self).map(
                            move |size, actor, _ctx| {
                                let should_render = (actor.view_model.episodes_list).handle_data(
                                    Versioned::new(PaginatedDataMessage::Size(
                                        size.unwrap().unwrap(),
                                    ))
                                    .with_version(version),
                                );
                                if should_render {
                                    actor.render();
                                }
                            },
                        ))
                    }
                    PaginatedDataRequest::Page { index, range } => Box::pin(
                        self.library
                            .send(QueryRequest {
                                data: query,
                                offset: range.start,
                                count: range.len(),
                            })
                            .into_actor(self)
                            .map(move |data, actor, _ctx| {
                                let should_render = (actor.view_model.episodes_list).handle_data(
                                    Versioned::new(PaginatedDataMessage::Page {
                                        index,
                                        values: data.unwrap().unwrap(),
                                    })
                                    .with_version(version),
                                );
                                if should_render {
                                    actor.render();
                                }
                            }),
                    ),
                }
            }
        }
    }
}

struct EpisodesListRowRenderer;

impl<'a> ListItemRenderingDelegate<'a> for EpisodesListRowRenderer {
    type Item = (Option<&'a hedgehog_library::model::EpisodeSummary>, bool);

    fn render_item(&self, area: Rect, item: Self::Item, buf: &mut tui::buffer::Buffer) {
        let (item, selected) = item;
        let style = if selected {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        match item {
            Some(item) => {
                let paragraph =
                    Paragraph::new(item.title.as_deref().unwrap_or("no title")).style(style);
                paragraph.render(area, buf);
            }
            None => buf.set_string(0, 0, " . . . ", style),
        }
    }
}
