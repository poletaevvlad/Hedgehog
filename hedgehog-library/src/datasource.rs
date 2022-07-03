use crate::actor::UpdateQuery;
use crate::metadata::{EpisodeMetadata, FeedMetadata};
use crate::model::{
    Episode, EpisodeId, EpisodePlaybackData, EpisodeStatus, EpisodeSummary, EpisodeSummaryStatus,
    EpisodesListMetadata, Feed, FeedId, FeedOMPLEntry, FeedStatus, FeedSummary, FeedView, GroupId,
    GroupSummary,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EpisodesQuery {
    pub(crate) episode_id: Option<EpisodeId>,
    pub(crate) feed_id: Option<FeedId>,
    pub(crate) group_id: Option<GroupId>,
    pub(crate) status: Option<EpisodeSummaryStatus>,
    pub(crate) with_hidden: bool,
    pub(crate) include_feed_title: bool,
    pub(crate) reversed_order: bool,
}

impl Default for EpisodesQuery {
    fn default() -> Self {
        Self {
            episode_id: None,
            feed_id: None,
            group_id: None,
            status: None,
            with_hidden: true,
            include_feed_title: false,
            reversed_order: false,
        }
    }
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

    pub fn group_id(mut self, group_id: GroupId) -> Self {
        self.group_id = Some(group_id);
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

    pub fn with_hidden(mut self, with_hidden: bool) -> Self {
        self.with_hidden = with_hidden;
        self
    }

    pub fn reversed_order(mut self, reversed_order: bool) -> Self {
        self.reversed_order = reversed_order;
        self
    }

    pub fn from_feed_view(feed_id: FeedView<FeedId, GroupId>) -> Self {
        match feed_id {
            FeedView::All => EpisodesQuery::default().include_feed_title(),
            FeedView::New => EpisodesQuery::default()
                .status(EpisodeSummaryStatus::New)
                .include_feed_title(),
            FeedView::Feed(feed_id) => EpisodesQuery::default().feed_id(feed_id),
            FeedView::Group(feed_id) => EpisodesQuery::default().group_id(feed_id),
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
    fn get_feed(&mut self, id: FeedId) -> DbResult<Option<Feed>>;
    fn get_feed_summaries(&mut self) -> DbResult<Vec<FeedSummary>>;
    fn get_feed_opml_entries(&mut self) -> DbResult<Vec<FeedOMPLEntry>>;
    fn get_update_sources(&mut self, update: UpdateQuery) -> DbResult<Vec<(FeedId, String)>>;
    fn get_new_episodes_count(
        &mut self,
        feed_ids: HashSet<FeedId>,
    ) -> DbResult<HashMap<FeedId, usize>>;
    fn rename_feed(&mut self, feed_id: FeedId, name: String) -> DbResult<()>;

    fn create_group(&mut self, name: &str) -> DbResult<Option<GroupId>>;
    fn get_group_summaries(&mut self) -> DbResult<Vec<GroupSummary>>;
    fn add_feed_to_group(&mut self, group_id: GroupId, feed_id: FeedId) -> DbResult<()>;
    fn rename_group(&mut self, group_id: GroupId, name: String) -> DbResult<()>;
    fn delete_group(&mut self, group_id: GroupId) -> DbResult<()>;
    fn set_group_position(&mut self, group_id: GroupId, position: usize) -> DbResult<()>;

    fn get_episode(&mut self, episode_id: EpisodeId) -> DbResult<Option<Episode>>;
    fn get_episode_playback_data(
        &mut self,
        episode_id: EpisodeId,
    ) -> DbResult<Option<EpisodePlaybackData>>;
    fn get_episodes_list_metadata(
        &mut self,
        query: EpisodesQuery,
    ) -> DbResult<EpisodesListMetadata>;
    fn get_episode_summaries(
        &mut self,
        query: EpisodesQuery,
        range: Range<usize>,
    ) -> DbResult<Vec<EpisodeSummary>>;
    fn count_episodes(&mut self, query: EpisodesQuery) -> DbResult<usize>;

    fn create_feed_pending(&mut self, data: &NewFeedMetadata) -> DbResult<Option<FeedId>>;
    fn delete_feed(&mut self, id: FeedId) -> DbResult<()>;
    fn set_feed_status(&mut self, feed_id: FeedId, status: FeedStatus) -> DbResult<()>;
    fn set_feed_enabled(&mut self, feed_id: FeedId, enabled: bool) -> DbResult<()>;
    fn reverse_feed_order(&mut self, feed_id: FeedId) -> DbResult<()>;

    fn set_episode_status(
        &mut self,
        query: EpisodesQuery,
        status: EpisodeStatus,
    ) -> DbResult<HashSet<FeedId>>;
    fn set_episode_hidden(&mut self, query: EpisodesQuery, hidden: bool) -> DbResult<()>;

    fn writer<'a>(&'a mut self, feed_id: FeedId) -> DbResult<Box<dyn EpisodeWriter + 'a>>;
}

pub trait EpisodeWriter {
    fn set_feed_metadata(&mut self, metadata: &FeedMetadata) -> DbResult<()>;
    fn set_episode_metadata(&mut self, metadata: &EpisodeMetadata) -> DbResult<EpisodeId>;
    fn delete_episode(&mut self, guid: &str) -> DbResult<()>;
    fn close(self: Box<Self>) -> DbResult<()>;
}
