use crate::{
    metadata::{EpisodeMetadata, FeedMetadata},
    model::{
        Episode, EpisodeId, EpisodeStatus, EpisodeSummary, Feed, FeedId, FeedStatus, FeedSummary,
    },
};
use std::marker::Unpin;
use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum QueryError {
    #[error(transparent)]
    SqliteError(#[from] rusqlite::Error),
}

pub type DbResult<T> = Result<T, QueryError>;

#[derive(Debug, Clone)]
pub enum EpisodeSummariesQuery {
    Single(EpisodeId),
    Multiple { feed_id: Option<FeedId> },
}

#[derive(Debug, PartialEq, Eq)]
pub struct Page {
    pub index: usize,
    pub size: usize,
}

impl Page {
    pub fn new(index: usize, size: usize) -> Self {
        Page { index, size }
    }

    pub(crate) fn offset(&self) -> usize {
        self.index * self.size
    }
}

pub trait DataProvider: Unpin {
    fn get_feed(&self, id: FeedId) -> DbResult<Option<Feed>>;
    fn get_feed_summaries(&self) -> DbResult<Vec<FeedSummary>>;

    fn get_episode(&self, episode_id: EpisodeId) -> DbResult<Option<Episode>>;
    fn get_episodes_count(&self, query: EpisodeSummariesQuery) -> DbResult<usize>;
    fn get_episode_summaries(
        &self,
        query: EpisodeSummariesQuery,
        page: Page,
    ) -> DbResult<Vec<EpisodeSummary>>;

    fn create_feed_pending(&self, source: &str) -> DbResult<FeedId>;
    fn delete_feed(&self, id: FeedId) -> DbResult<()>;
    fn set_feed_status(&self, feed_id: FeedId, status: FeedStatus) -> DbResult<()>;
    fn get_feed_source(&self, id: FeedId) -> DbResult<String>;
    fn set_episode_status(
        &self,
        query: EpisodeSummariesQuery,
        status: EpisodeStatus,
    ) -> DbResult<()>;
}

pub trait WritableDataProvider {
    type Writer: EpisodeWriter;

    fn writer(self, feed_id: FeedId) -> DbResult<Self::Writer>;
}

pub trait EpisodeWriter {
    fn set_feed_metadata(&mut self, metadata: &FeedMetadata) -> DbResult<()>;
    fn set_episode_metadata(&mut self, metadata: &EpisodeMetadata) -> DbResult<EpisodeId>;
    fn close(self) -> DbResult<()>;
}
