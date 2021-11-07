use crate::{
    metadata::{EpisodeMetadata, FeedMetadata},
    model::{Episode, EpisodeId, EpisodeSummary, Feed, FeedId, FeedStatus, FeedSummary},
};
use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum QueryError {
    #[error(transparent)]
    SqliteError(#[from] rusqlite::Error),
}

pub trait ListQuery: Send {
    type Item: 'static + Send;
}

pub trait PagedQueryHandler<P: ListQuery> {
    fn get_size(&self, request: P) -> Result<usize, QueryError>;

    fn query_page(
        &self,
        request: P,
        offset: usize,
        count: usize,
    ) -> Result<Vec<P::Item>, QueryError>;
}

pub trait QueryHandler<P: ListQuery> {
    fn query(&self, request: P) -> Result<Vec<P::Item>, QueryError>;
}

#[derive(Debug, Clone)]
pub struct FeedSummariesQuery;

impl ListQuery for FeedSummariesQuery {
    type Item = FeedSummary;
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

impl ListQuery for EpisodeSummariesQuery {
    type Item = EpisodeSummary;
}

pub trait DataProvider:
    std::marker::Unpin + QueryHandler<FeedSummariesQuery> + PagedQueryHandler<EpisodeSummariesQuery>
{
    fn get_feed(&self, id: FeedId) -> Result<Option<Feed>, QueryError>;
    fn get_episode(&self, episode_id: EpisodeId) -> Result<Option<Episode>, QueryError>;
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
