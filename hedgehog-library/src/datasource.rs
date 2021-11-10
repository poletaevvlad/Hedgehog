use crate::{
    metadata::{EpisodeMetadata, FeedMetadata},
    model::{Episode, EpisodeId, EpisodeSummary, Feed, FeedId, FeedStatus, FeedSummary},
};
use std::marker::Unpin;
use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum QueryError {
    #[error(transparent)]
    SqliteError(#[from] rusqlite::Error),
}

#[derive(Default, Debug, Clone)]
pub struct EpisodeSummariesQuery {
    pub feed_id: Option<FeedId>,
}

impl EpisodeSummariesQuery {
    pub fn with_feed_id(mut self, feed_id: impl Into<Option<FeedId>>) -> Self {
        self.feed_id = feed_id.into();
        self
    }
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
    fn get_feed(&self, id: FeedId) -> Result<Option<Feed>, QueryError>;
    fn get_feed_summaries(&self) -> Result<Vec<FeedSummary>, QueryError>;

    fn get_episode(&self, episode_id: EpisodeId) -> Result<Option<Episode>, QueryError>;
    fn get_episodes_count(&self, query: EpisodeSummariesQuery) -> Result<usize, QueryError>;
    fn get_episode_summaries(
        &self,
        query: EpisodeSummariesQuery,
        page: Page,
    ) -> Result<Vec<EpisodeSummary>, QueryError>;

    fn create_feed_pending(&self, source: &str) -> Result<FeedId, QueryError>;
    fn delete_feed(&self, id: FeedId) -> Result<(), QueryError>;
    fn set_feed_status(&self, feed_id: FeedId, status: FeedStatus) -> Result<(), QueryError>;
    fn get_feed_source(&self, id: FeedId) -> Result<String, QueryError>;
}

pub trait WritableDataProvider {
    type Writer: EpisodeWriter;

    fn writer(self, feed_id: FeedId) -> Result<Self::Writer, QueryError>;
}

pub trait EpisodeWriter {
    fn set_feed_metadata(&mut self, metadata: &FeedMetadata) -> Result<(), QueryError>;
    fn set_episode_metadata(&mut self, metadata: &EpisodeMetadata)
        -> Result<EpisodeId, QueryError>;
    fn close(self) -> Result<(), QueryError>;
}
