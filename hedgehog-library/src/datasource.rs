use crate::metadata::{EpisodeMetadata, FeedMetadata};
use crate::model::{
    Episode, EpisodeId, EpisodePlaybackData, EpisodeStatus, EpisodeSummary, EpisodesListMetadata,
    Feed, FeedId, FeedStatus, FeedSummary, FeedView,
};
use std::marker::Unpin;
use std::ops::Range;
use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum QueryError {
    #[error(transparent)]
    SqliteError(#[from] rusqlite::Error),
}

pub type DbResult<T> = Result<T, QueryError>;

#[derive(Debug, Clone)]
pub enum EpisodesQuery {
    Single(EpisodeId),
    Multiple {
        feed_id: Option<FeedId>,
        include_feed_title: bool,
    },
}

impl EpisodesQuery {
    pub fn from_feed_view(feed_id: FeedView<FeedId>) -> Self {
        match feed_id {
            FeedView::All => EpisodesQuery::Multiple {
                feed_id: None,
                include_feed_title: true,
            },
            FeedView::Feed(feed_id) => EpisodesQuery::Multiple {
                feed_id: Some(feed_id),
                include_feed_title: false,
            },
        }
    }
}

pub trait DataProvider: Unpin {
    fn get_feed(&self, id: FeedId) -> DbResult<Option<Feed>>;
    fn get_feed_summaries(&self) -> DbResult<Vec<FeedSummary>>;
    fn get_update_sources(&self) -> DbResult<Vec<(FeedId, String)>>;

    fn get_episode(&self, episode_id: EpisodeId) -> DbResult<Option<Episode>>;
    fn get_episode_playback_data(&self, episode_id: EpisodeId) -> DbResult<EpisodePlaybackData>;
    fn get_episodes_list_metadata(&self, query: EpisodesQuery) -> DbResult<EpisodesListMetadata>;
    fn get_episode_summaries(
        &self,
        query: EpisodesQuery,
        range: Range<usize>,
    ) -> DbResult<Vec<EpisodeSummary>>;

    fn create_feed_pending(&self, source: &str) -> DbResult<FeedId>;
    fn delete_feed(&self, id: FeedId) -> DbResult<()>;
    fn set_feed_status(&self, feed_id: FeedId, status: FeedStatus) -> DbResult<()>;
    fn set_feed_enabled(&self, feed_id: FeedId, enabled: bool) -> DbResult<()>;
    fn get_feed_source(&self, id: FeedId) -> DbResult<String>;

    fn set_episode_status(&self, query: EpisodesQuery, status: EpisodeStatus) -> DbResult<()>;
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
