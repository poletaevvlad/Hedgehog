use crate::model::{EpisodeId, EpisodeStatus};
use crate::{EpisodesQuery, FeedUpdateRequest, Library};
use actix::prelude::*;

pub struct StatusWriter {
    library: Addr<Library>,
}

impl StatusWriter {
    pub fn new(library: Addr<Library>) -> Self {
        StatusWriter { library }
    }
}

impl Actor for StatusWriter {
    type Context = Context<Self>;
}

#[derive(Message)]
#[rtype(result = "()")]
pub enum StatusWriterCommand {
    Set(EpisodeId, EpisodeStatus),
}

impl Handler<StatusWriterCommand> for StatusWriter {
    type Result = ();

    fn handle(&mut self, msg: StatusWriterCommand, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            StatusWriterCommand::Set(episode_id, status) => self.library.do_send(
                FeedUpdateRequest::SetStatus(EpisodesQuery::Single(episode_id), status),
            ),
        }
    }
}