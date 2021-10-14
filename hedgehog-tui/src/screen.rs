use crate::cmdparser;
use crate::events::key;
use crate::history::CommandsHistory;
use crate::paging::{InteractiveList, PaginatedDataProvider, PaginatedList};
use crate::status::{Severity, Status};
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
    status: Option<Status>,
    library: Addr<Library>,
    episodes_list:
        Option<InteractiveList<hedgehog_library::model::EpisodeSummary, EpisodesListProvider>>,
}

impl UI {
    pub(crate) fn new(
        terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
        library: Addr<Library>,
    ) -> Self {
        UI {
            terminal,
            command: None,
            commands_history: CommandsHistory::new(),
            status: None,
            library,
            episodes_list: None,
        }
    }

    fn render(&mut self) {
        let command = &mut self.command;
        let history = &self.commands_history;
        let status = &self.status;
        let episodes_list = &self.episodes_list;

        let draw = |f: &mut tui::Frame<CrosstermBackend<std::io::Stdout>>| {
            let area = f.size();
            let library_rect = Rect::new(0, 0, area.width, area.height - 1);
            if let Some(list) = episodes_list {
                f.render_widget(
                    List::new(EpisodesListRowRenderer, list.items.iter()),
                    library_rect,
                );
            }

            let status_rect = Rect::new(0, area.height - 1, area.width, 1);
            if let Some(ref mut command_state) = command {
                CommandEditor::new(command_state)
                    .prefix(Span::raw(":"))
                    .render(f, status_rect, history);
            } else if let Some(status) = status {
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
        ctx.address()
            .do_send(EpisodesSizeRequest(EpisodeSummariesQuery { feed_id: None }));
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
            Ok(Event::Resize(_, _)) => {
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
                    System::current().stop();
                    false
                }
                key!(':') => {
                    self.status = None;
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
                        match cmdparser::from_str::<()>(&command_str) {
                            Ok(_cmd) => (),
                            Err(error) => self.status = Some(error.into()),
                        };
                        self.commands_history.push(&command_str);
                        self.command = None;
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

struct EpisodesListProvider {
    query: EpisodeSummariesQuery,
    actor: Addr<UI>,
}

impl PaginatedDataProvider for EpisodesListProvider {
    fn request_page(&mut self, index: usize, offset: usize, size: usize) {
        self.actor.clone().do_send(EpisodesDataRequest {
            index,
            offset,
            count: size,
            request: self.query.clone(),
        })
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct EpisodesDataRequest {
    index: usize,
    count: usize,
    offset: usize,
    request: EpisodeSummariesQuery,
}

impl Handler<EpisodesDataRequest> for UI {
    type Result = ResponseActFuture<Self, ()>;

    fn handle(&mut self, msg: EpisodesDataRequest, _ctx: &mut Self::Context) -> Self::Result {
        let index = msg.index;
        let result_future = self
            .library
            .send(QueryRequest::new(msg.request, msg.count).with_offset(msg.offset))
            .into_actor(self)
            .map(move |result, actor, _ctx| {
                if let Some(ref mut list) = actor.episodes_list {
                    list.items.data_available(index, result.unwrap().unwrap())
                }
                actor.render();
            });
        Box::pin(result_future)
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct EpisodesSizeRequest(EpisodeSummariesQuery);

impl Handler<EpisodesSizeRequest> for UI {
    type Result = ResponseActFuture<Self, ()>;

    fn handle(&mut self, msg: EpisodesSizeRequest, _ctx: &mut Self::Context) -> Self::Result {
        let query = msg.0;
        let result_future = self
            .library
            .send(SizeRequest(query.clone()))
            .into_actor(self)
            .map(move |result, actor, ctx| {
                let list = PaginatedList::new(
                    32,
                    result.unwrap().unwrap(),
                    EpisodesListProvider {
                        query,
                        actor: ctx.address(),
                    },
                );
                actor.episodes_list = Some(InteractiveList::new(list));
                actor.render();
            });
        Box::pin(result_future)
    }
}

struct EpisodesListRowRenderer;

impl<'a> ListItemRenderingDelegate<'a> for EpisodesListRowRenderer {
    type Item = Option<&'a hedgehog_library::model::EpisodeSummary>;

    fn render_item(&self, area: Rect, item: Self::Item, buf: &mut tui::buffer::Buffer) {
        match item {
            Some(item) => {
                let paragraph = Paragraph::new(item.title.as_deref().unwrap_or("no title"));
                paragraph.render(area, buf);
            }
            None => buf.set_string(0, 0, " . . . ", Style::default()),
        }
    }
}
