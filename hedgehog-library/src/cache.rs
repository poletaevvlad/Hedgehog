use crate::datasource::{DataProvider, DbResult, EpisodeWriter};
use crate::model::{
    Episode, EpisodeId, EpisodePlaybackData, EpisodeStatus, EpisodeSummary, EpisodesListMetadata,
    Feed, FeedId, FeedOMPLEntry, FeedStatus, FeedSummary, GroupId,
};
use crate::{EpisodesQuery, NewFeedMetadata, UpdateQuery};
use std::collections::{HashMap, HashSet};
use std::ops::Range;

pub struct InMemoryCache<D> {
    data_provider: D,
    episodes_list_metadata: HashMap<EpisodesQuery, EpisodesListMetadata>,
    episodes_summaries: HashMap<EpisodesQuery, HashMap<Range<usize>, Vec<EpisodeSummary>>>,
}

impl<D> InMemoryCache<D> {
    pub fn new(data_provider: D) -> Self {
        InMemoryCache {
            data_provider,
            episodes_list_metadata: HashMap::new(),
            episodes_summaries: HashMap::new(),
        }
    }

    fn invalidate_where(&mut self, pred: impl Fn(&EpisodesQuery) -> bool) {
        self.episodes_list_metadata.retain(|key, _| !pred(key));
        self.episodes_summaries.retain(|key, _| !pred(key));
    }

    fn invalidate_feed(&mut self, feed_id: FeedId) {
        self.invalidate_where(|query| match query.feed_id {
            Some(query_feed_id) => query_feed_id == feed_id,
            None => true,
        });
    }

    fn invalidate_all(&mut self) {
        self.episodes_list_metadata.clear();
        self.episodes_summaries.clear();
    }
}

impl<D: DataProvider> DataProvider for InMemoryCache<D> {
    fn get_feed(&mut self, id: FeedId) -> DbResult<Option<Feed>> {
        self.data_provider.get_feed(id)
    }

    fn get_feed_summaries(&mut self) -> DbResult<Vec<FeedSummary>> {
        self.data_provider.get_feed_summaries()
    }

    fn get_feed_opml_entries(&mut self) -> DbResult<Vec<FeedOMPLEntry>> {
        self.data_provider.get_feed_opml_entries()
    }

    fn get_update_sources(&mut self, update: UpdateQuery) -> DbResult<Vec<(FeedId, String)>> {
        self.data_provider.get_update_sources(update)
    }

    fn get_new_episodes_count(
        &mut self,
        feed_ids: HashSet<FeedId>,
    ) -> DbResult<HashMap<FeedId, usize>> {
        self.data_provider.get_new_episodes_count(feed_ids)
    }

    fn rename_feed(&mut self, feed_id: FeedId, name: String) -> DbResult<()> {
        self.data_provider.rename_feed(feed_id, name)
    }

    fn create_group(&mut self, name: &str) -> DbResult<Option<GroupId>> {
        self.data_provider.create_group(name)
    }

    fn get_group_summaries(&mut self) -> DbResult<Vec<crate::model::GroupSummary>> {
        self.data_provider.get_group_summaries()
    }

    fn add_feed_to_group(&mut self, group_id: GroupId, feed_id: FeedId) -> DbResult<()> {
        self.data_provider.add_feed_to_group(group_id, feed_id)
    }

    fn rename_group(&mut self, group_id: GroupId, name: String) -> DbResult<()> {
        self.data_provider.rename_group(group_id, name)
    }

    fn delete_group(&mut self, group_id: GroupId) -> DbResult<()> {
        self.data_provider.delete_group(group_id)
    }

    fn set_group_position(&mut self, group_id: GroupId, position: usize) -> DbResult<()> {
        self.data_provider.set_group_position(group_id, position)
    }

    fn get_episode(&mut self, episode_id: EpisodeId) -> DbResult<Option<Episode>> {
        self.data_provider.get_episode(episode_id)
    }

    fn get_episode_playback_data(
        &mut self,
        episode_id: EpisodeId,
    ) -> DbResult<Option<EpisodePlaybackData>> {
        self.data_provider.get_episode_playback_data(episode_id)
    }

    fn get_episodes_list_metadata(
        &mut self,
        query: EpisodesQuery,
    ) -> DbResult<EpisodesListMetadata> {
        match self.episodes_list_metadata.get(&query).cloned() {
            Some(metadata) => Ok(metadata),
            None => {
                let metadata = self
                    .data_provider
                    .get_episodes_list_metadata(query.clone())?;
                self.episodes_list_metadata.insert(query, metadata.clone());
                Ok(metadata)
            }
        }
    }

    fn get_episode_summaries(
        &mut self,
        query: EpisodesQuery,
        range: Range<usize>,
    ) -> DbResult<Vec<EpisodeSummary>> {
        match self.episodes_summaries.get_mut(&query) {
            Some(list_items) => match list_items.get(&range) {
                Some(items) => Ok(items.clone()),
                None => {
                    let summaries = self
                        .data_provider
                        .get_episode_summaries(query.clone(), range.clone())?;
                    list_items.insert(range, summaries.clone());
                    Ok(summaries)
                }
            },
            None => {
                let summaries = self
                    .data_provider
                    .get_episode_summaries(query.clone(), range.clone())?;
                let mut items = HashMap::new();
                items.insert(range, summaries.clone());
                self.episodes_summaries.insert(query, items);
                Ok(summaries)
            }
        }
    }

    fn count_episodes(&mut self, query: EpisodesQuery) -> DbResult<usize> {
        self.data_provider.count_episodes(query)
    }

    fn create_feed_pending(&mut self, data: &NewFeedMetadata) -> DbResult<Option<FeedId>> {
        self.data_provider.create_feed_pending(data)
    }

    fn delete_feed(&mut self, id: FeedId) -> DbResult<()> {
        self.data_provider.delete_feed(id)?;
        self.invalidate_feed(id);
        Ok(())
    }

    fn set_feed_status(&mut self, feed_id: FeedId, status: FeedStatus) -> DbResult<()> {
        self.data_provider.set_feed_status(feed_id, status)
    }

    fn set_feed_enabled(&mut self, feed_id: FeedId, enabled: bool) -> DbResult<()> {
        self.data_provider.set_feed_enabled(feed_id, enabled)
    }

    fn reverse_feed_order(&mut self, feed_id: FeedId) -> DbResult<()> {
        self.invalidate_feed(feed_id);
        self.data_provider.reverse_feed_order(feed_id)
    }

    fn set_episode_status(
        &mut self,
        query: EpisodesQuery,
        status: EpisodeStatus,
    ) -> DbResult<HashSet<FeedId>> {
        let ids = self.data_provider.set_episode_status(query, status)?;
        for id in &ids {
            self.invalidate_feed(*id);
        }
        Ok(ids)
    }

    fn set_episode_hidden(&mut self, query: EpisodesQuery, hidden: bool) -> DbResult<()> {
        self.invalidate_all();
        self.data_provider.set_episode_hidden(query, hidden)
    }

    fn writer<'a>(&'a mut self, feed_id: FeedId) -> DbResult<Box<dyn EpisodeWriter + 'a>> {
        self.invalidate_feed(feed_id);
        self.data_provider.writer(feed_id)
    }
}
