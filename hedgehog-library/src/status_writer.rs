use crate::model::{EpisodeId, EpisodeStatus};
use crate::{EpisodesQuery, FeedUpdateRequest, Library};
use actix::prelude::*;
use std::fs;
use std::io::{BufRead, BufReader, ErrorKind};
use std::path::PathBuf;
use std::time::Duration;

pub struct StatusWriter {
    library: Addr<Library>,
    playing_path: Option<PathBuf>,
    saved_episode_id: Option<EpisodeId>,
}

impl StatusWriter {
    pub fn new(library: Addr<Library>) -> Self {
        StatusWriter {
            library,
            playing_path: None,
            saved_episode_id: None,
        }
    }

    pub fn set_playing_path(mut self, path: PathBuf) -> Self {
        self.saved_episode_id = match fs::File::open(&path) {
            Ok(file) => {
                let mut buffer = String::new();
                if let Err(err) = BufReader::new(file).read_line(&mut buffer) {
                    log::error!(target:"io", "Cannot load previous playback status: {}", err);
                    None
                } else {
                    let id = buffer.trim_end().parse::<i64>().ok().map(EpisodeId);
                    id
                }
            }
            Err(err) if err.kind() == ErrorKind::NotFound => None,
            Err(err) => {
                log::error!(target:"io", "Cannot load previous playback status: {}", err);
                None
            }
        };
        self
    }

    fn save_episode_id(&mut self, episode_id: Option<EpisodeId>) {
        if episode_id == self.saved_episode_id {
            return;
        }
        if let Some(ref playing_path) = self.playing_path {
            let result = match episode_id {
                Some(episode_id) => fs::write(&playing_path, format!("{}\n", episode_id.as_i64())),
                None => std::fs::remove_file(&playing_path),
            };

            match result {
                Ok(_) => {
                    self.saved_episode_id = episode_id;
                }
                Err(error) => {
                    self.saved_episode_id = None;
                    self.playing_path = None;
                    log::error!(target:"io", "Cannot save playback status: {}", error);
                }
            }
        }
    }
}

impl Actor for StatusWriter {
    type Context = Context<Self>;
}

#[derive(Message)]
#[rtype(result = "()")]
pub enum StatusWriterCommand {
    Set(EpisodesQuery, EpisodeStatus),
    StopPlayback,
}

impl StatusWriterCommand {
    pub fn set(episode_id: EpisodeId, status: EpisodeStatus) -> Self {
        StatusWriterCommand::Set(EpisodesQuery::default().id(episode_id), status)
    }

    pub fn set_finished(episode_id: EpisodeId) -> Self {
        StatusWriterCommand::Set(
            EpisodesQuery::default().id(episode_id),
            EpisodeStatus::Finished,
        )
    }

    pub fn set_position(episode_id: EpisodeId, position: Duration) -> Self {
        StatusWriterCommand::Set(
            EpisodesQuery::default().id(episode_id),
            EpisodeStatus::Started(position),
        )
    }

    pub fn set_error(episode_id: EpisodeId, position: Duration) -> Self {
        StatusWriterCommand::Set(
            EpisodesQuery::default().id(episode_id),
            EpisodeStatus::Error(position),
        )
    }
}

impl Handler<StatusWriterCommand> for StatusWriter {
    type Result = ();

    fn handle(&mut self, msg: StatusWriterCommand, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            StatusWriterCommand::Set(query, status) => {
                if let Some(episode_id) = query.episode_id {
                    let future_saved = match status {
                        EpisodeStatus::Finished => Some(None),
                        EpisodeStatus::Started(_) => Some(Some(episode_id)),
                        _ => None,
                    };
                    if let Some(future_saved) = future_saved {
                        self.save_episode_id(future_saved);
                    }
                }

                self.library
                    .do_send(FeedUpdateRequest::SetStatus(query, status));
            }
            StatusWriterCommand::StopPlayback => self.save_episode_id(None),
        }
    }
}

#[derive(Message)]
#[rtype("Option<EpisodeId>")]
pub struct GetPlayingEpisodeId;

impl Handler<GetPlayingEpisodeId> for StatusWriter {
    type Result = Option<EpisodeId>;

    fn handle(&mut self, _: GetPlayingEpisodeId, _ctx: &mut Self::Context) -> Self::Result {
        self.saved_episode_id
    }
}
