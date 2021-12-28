use crate::actor::UpdateQuery;
use crate::metadata::{EpisodeMetadata, FeedMetadata};
use crate::model::{
    Episode, EpisodeId, EpisodePlaybackData, EpisodeStatus, EpisodeSummary, EpisodeSummaryStatus,
    EpisodesListMetadata, Feed, FeedId, FeedOMPLEntry, FeedStatus, FeedSummary, FeedView,
};
use std::collections::{HashMap, HashSet};
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

#[derive(Default, Debug, Clone)]
pub struct EpisodesQuery {
    pub(crate) episode_id: Option<EpisodeId>,
    pub(crate) feed_id: Option<FeedId>,
    pub(crate) status: Option<EpisodeSummaryStatus>,
    pub(crate) include_feed_title: bool,
}

impl EpisodesQuery {
    pub fn id(mut self, episode_id: EpisodeId) -> Self {
        self.episode_id = Some(episode_id);
        self
    }

    pub fn feed_id(mut self, feed_id: FeedId) -> Self {
        self.feed_id = Some(feed_id);
        self
    }

    pub fn status(mut self, status: EpisodeSummaryStatus) -> Self {
        self.status = Some(status);
        self
    }

    pub fn include_feed_title(mut self) -> Self {
        self.include_feed_title = true;
        self
    }

    pub fn from_feed_view(feed_id: FeedView<FeedId>) -> Self {
        match feed_id {
            FeedView::All => EpisodesQuery::default().include_feed_title(),
            FeedView::New => EpisodesQuery::default()
                .status(EpisodeSummaryStatus::New)
                .include_feed_title(),
            FeedView::Feed(feed_id) => EpisodesQuery::default().feed_id(feed_id),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct NewFeedMetadata {
    pub(crate) source: String,
    pub(crate) title: Option<String>,
    pub(crate) link: Option<String>,
}

impl NewFeedMetadata {
    pub fn new(source: String) -> Self {
        NewFeedMetadata {
            source,
            title: None,
            link: None,
        }
    }

    pub fn with_title(mut self, title: impl Into<Option<String>>) -> Self {
        self.title = title.into();
        self
    }

    pub fn with_link(mut self, link: impl Into<Option<String>>) -> Self {
        self.link = link.into();
        self
    }
}

pub trait DataProvider: Unpin {
    fn get_feed(&self, id: FeedId) -> DbResult<Option<Feed>>;
    fn get_feed_summaries(&self) -> DbResult<Vec<FeedSummary>>;
    fn get_feed_opml_entries(&self) -> DbResult<Vec<FeedOMPLEntry>>;
    fn get_update_sources(&self, update: UpdateQuery) -> DbResult<Vec<(FeedId, String)>>;
    fn get_new_episodes_count(
        &self,
        feed_ids: impl IntoIterator<Item = FeedId>,
    ) -> DbResult<HashMap<FeedId, usize>>;

    fn get_episode(&self, episode_id: EpisodeId) -> DbResult<Option<Episode>>;
    fn get_episode_playback_data(&self, episode_id: EpisodeId) -> DbResult<EpisodePlaybackData>;
    fn get_episodes_list_metadata(&self, query: EpisodesQuery) -> DbResult<EpisodesListMetadata>;
    fn get_episode_summaries(
        &self,
        query: EpisodesQuery,
        range: Range<usize>,
    ) -> DbResult<Vec<EpisodeSummary>>;
    fn count_episodes(&self, query: EpisodesQuery) -> DbResult<usize>;

    fn create_feed_pending(&self, data: &NewFeedMetadata) -> DbResult<Option<FeedId>>;
    fn delete_feed(&self, id: FeedId) -> DbResult<()>;
    fn set_feed_status(&self, feed_id: FeedId, status: FeedStatus) -> DbResult<()>;
    fn set_feed_enabled(&self, feed_id: FeedId, enabled: bool) -> DbResult<()>;

    fn set_episode_status(
        &self,
        query: EpisodesQuery,
        status: EpisodeStatus,
    ) -> DbResult<HashSet<FeedId>>;
}

pub trait WritableDataProvider {
    type Writer: EpisodeWriter;

    fn writer(self, feed_id: FeedId) -> DbResult<Self::Writer>;
}

pub trait EpisodeWriter {
    fn set_feed_metadata(&mut self, metadata: &FeedMetadata) -> DbResult<()>;
    fn set_episode_metadata(&mut self, metadata: &EpisodeMetadata) -> DbResult<EpisodeId>;
    fn delete_episode(&mut self, guid: &str) -> DbResult<()>;
    fn close(self) -> DbResult<()>;
}
