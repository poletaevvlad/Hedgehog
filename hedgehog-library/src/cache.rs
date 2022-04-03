use crate::datasource::{DataProvider, DbResult, EpisodeWriter};
use crate::model::{
    Episode, EpisodeId, EpisodePlaybackData, EpisodeStatus, EpisodeSummary, EpisodesListMetadata,
    Feed, FeedId, FeedOMPLEntry, FeedStatus, FeedSummary,
};
use crate::{EpisodesQuery, NewFeedMetadata, UpdateQuery};
use std::collections::{HashMap, HashSet};
use std::ops::Range;

pub struct InMemoryCache<D> {
    data_provider: D,
}

impl<D> InMemoryCache<D> {
    pub fn new(data_provider: D) -> Self {
        InMemoryCache { data_provider }
    }
}

impl<D: DataProvider> DataProvider for InMemoryCache<D> {
    fn get_feed(&self, id: FeedId) -> DbResult<Option<Feed>> {
        self.data_provider.get_feed(id)
    }

    fn get_feed_summaries(&self) -> DbResult<Vec<FeedSummary>> {
        self.data_provider.get_feed_summaries()
    }

    fn get_feed_opml_entries(&self) -> DbResult<Vec<FeedOMPLEntry>> {
        self.data_provider.get_feed_opml_entries()
    }

    fn get_update_sources(&self, update: UpdateQuery) -> DbResult<Vec<(FeedId, String)>> {
        self.data_provider.get_update_sources(update)
    }

    fn get_new_episodes_count(
        &self,
        feed_ids: HashSet<FeedId>,
    ) -> DbResult<HashMap<FeedId, usize>> {
        self.data_provider.get_new_episodes_count(feed_ids)
    }

    fn get_episode(&self, episode_id: EpisodeId) -> DbResult<Option<Episode>> {
        self.data_provider.get_episode(episode_id)
    }

    fn get_episode_playback_data(&self, episode_id: EpisodeId) -> DbResult<EpisodePlaybackData> {
        self.data_provider.get_episode_playback_data(episode_id)
    }

    fn get_episodes_list_metadata(&self, query: EpisodesQuery) -> DbResult<EpisodesListMetadata> {
        self.data_provider.get_episodes_list_metadata(query)
    }

    fn get_episode_summaries(
        &self,
        query: EpisodesQuery,
        range: Range<usize>,
    ) -> DbResult<Vec<EpisodeSummary>> {
        self.data_provider.get_episode_summaries(query, range)
    }

    fn count_episodes(&self, query: EpisodesQuery) -> DbResult<usize> {
        self.data_provider.count_episodes(query)
    }

    fn create_feed_pending(&self, data: &NewFeedMetadata) -> DbResult<Option<FeedId>> {
        self.data_provider.create_feed_pending(data)
    }

    fn delete_feed(&self, id: FeedId) -> DbResult<()> {
        self.data_provider.delete_feed(id)
    }

    fn set_feed_status(&self, feed_id: FeedId, status: FeedStatus) -> DbResult<()> {
        self.data_provider.set_feed_status(feed_id, status)
    }

    fn set_feed_enabled(&self, feed_id: FeedId, enabled: bool) -> DbResult<()> {
        self.data_provider.set_feed_enabled(feed_id, enabled)
    }

    fn reverse_feed_order(&self, feed_id: FeedId) -> DbResult<()> {
        self.data_provider.reverse_feed_order(feed_id)
    }

    fn set_episode_status(
        &self,
        query: EpisodesQuery,
        status: EpisodeStatus,
    ) -> DbResult<HashSet<FeedId>> {
        self.data_provider.set_episode_status(query, status)
    }

    fn set_episode_hidden(&self, query: EpisodesQuery, hidden: bool) -> DbResult<()> {
        self.data_provider.set_episode_hidden(query, hidden)
    }

    fn writer<'a>(&'a mut self, feed_id: FeedId) -> DbResult<Box<dyn EpisodeWriter + 'a>> {
        self.data_provider.writer(feed_id)
    }
}
