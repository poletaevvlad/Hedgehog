use crate::datasource::{DataProvider, NewFeedMetadata, QueryError};
use crate::model::{
    Episode, EpisodeId, EpisodePlaybackData, EpisodeStatus, EpisodeSummary, EpisodeSummaryStatus,
    EpisodesListMetadata, Feed, FeedId, FeedStatus, FeedSummary, GroupId, GroupSummary,
};
use crate::rss_client::{fetch_feed, WritableFeed};
use crate::EpisodesQuery;
use actix::dev::MessageResponse;
use actix::fut::wrap_future;
use actix::prelude::*;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::Arc;
use tokio::sync::Semaphore;

pub struct Library {
    data_provider: Box<dyn DataProvider>,
    updating_feeds: HashSet<FeedId>,
    feeds_semaphore: Arc<Semaphore>,
    update_listener: Option<Recipient<FeedUpdateNotification>>,
}

impl Library {
    pub fn new(data_provider: impl DataProvider + 'static) -> Self {
        Library {
            data_provider: Box::new(data_provider),
            updating_feeds: HashSet::new(),
            feeds_semaphore: Arc::new(Semaphore::new(8)),
            update_listener: None,
        }
    }
}

impl Actor for Library {
    type Context = Context<Self>;
}

#[derive(Message)]
#[rtype(result = "Vec<EpisodeSummary>")]
pub struct EpisodeSummariesRequest {
    pub query: EpisodesQuery,
    pub range: Range<usize>,
}

impl EpisodeSummariesRequest {
    pub fn new(query: EpisodesQuery, range: Range<usize>) -> Self {
        EpisodeSummariesRequest { query, range }
    }
}

impl Handler<EpisodeSummariesRequest> for Library {
    type Result = Vec<EpisodeSummary>;

    fn handle(&mut self, msg: EpisodeSummariesRequest, _ctx: &mut Self::Context) -> Self::Result {
        let result = self
            .data_provider
            .get_episode_summaries(msg.query, msg.range);
        match result {
            Ok(summaries) => summaries,
            Err(error) => {
                log::error!(target: "sql", "cannot fetch episode summaries, {}", error);
                Vec::new()
            }
        }
    }
}

#[derive(Message)]
#[rtype(result = "EpisodesListMetadata")]
pub struct EpisodesListMetadataRequest(pub EpisodesQuery);

impl Handler<EpisodesListMetadataRequest> for Library {
    type Result = EpisodesListMetadata;

    fn handle(
        &mut self,
        msg: EpisodesListMetadataRequest,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        match self.data_provider.get_episodes_list_metadata(msg.0) {
            Ok(result) => result,
            Err(error) => {
                log::error!(target: "sql", "cannot fetch episodes list metadata, {}", error);
                EpisodesListMetadata::default()
            }
        }
    }
}

#[derive(MessageResponse)]
pub struct FeedSummariesResponse {
    pub feeds: Vec<FeedSummary>,
    pub groups: Vec<GroupSummary>,
}

#[derive(Message)]
#[rtype(result = "FeedSummariesResponse")]
pub struct FeedSummariesRequest;

impl Handler<FeedSummariesRequest> for Library {
    type Result = FeedSummariesResponse;

    fn handle(&mut self, _msg: FeedSummariesRequest, _ctx: &mut Self::Context) -> Self::Result {
        let feeds = self
            .data_provider
            .get_feed_summaries()
            .unwrap_or_else(|error| {
                log::error!(target: "sql", "cannot fetch feed summaries, {}", error);
                Vec::new()
            });
        let groups = self
            .data_provider
            .get_group_summaries()
            .unwrap_or_else(|error| {
                log::error!(target: "sql", "cannot fetch group summaries, {}", error);
                Vec::new()
            });
        FeedSummariesResponse { feeds, groups }
    }
}

#[derive(Message)]
#[rtype(result = "Option<EpisodePlaybackData>")]
pub struct EpisodePlaybackDataRequest(pub EpisodeId);

impl Handler<EpisodePlaybackDataRequest> for Library {
    type Result = Option<EpisodePlaybackData>;

    fn handle(
        &mut self,
        msg: EpisodePlaybackDataRequest,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        match self.data_provider.get_episode_playback_data(msg.0) {
            Ok(result) => result,
            Err(error) => {
                log::error!(target: "sql", "cannot get episode playback data, {}", error);
                None
            }
        }
    }
}

#[derive(Message)]
#[rtype(result = "Option<Episode>")]
pub struct EpisodeRequest(pub EpisodeId);

impl Handler<EpisodeRequest> for Library {
    type Result = Option<Episode>;

    fn handle(&mut self, msg: EpisodeRequest, _ctx: &mut Self::Context) -> Self::Result {
        match self.data_provider.get_episode(msg.0) {
            Ok(result) => result,
            Err(error) => {
                log::error!(target: "sql", "cannot fetch episode, {}", error);
                None
            }
        }
    }
}

#[derive(Message)]
#[rtype(result = "Option<Feed>")]
pub struct FeedRequest(pub FeedId);

impl Handler<FeedRequest> for Library {
    type Result = Option<Feed>;

    fn handle(&mut self, msg: FeedRequest, _ctx: &mut Self::Context) -> Self::Result {
        match self.data_provider.get_feed(msg.0) {
            Ok(result) => result,
            Err(error) => {
                log::error!(target: "sql", "cannot fetch feed, {}", error);
                None
            }
        }
    }
}

impl Library {
    fn notify_update_listener(&mut self, message: FeedUpdateNotification) {
        if let Some(listener) = &self.update_listener {
            let result = listener.do_send(message);
            if let Err(SendError::Closed(_)) = result {
                self.update_listener = None;
            }
        }
    }

    fn schedule_update(
        &mut self,
        mut feeds: Vec<(FeedId, String)>,
        ctx: &mut <Library as Actor>::Context,
    ) {
        feeds.retain(|(feed_id, _)| !self.updating_feeds.contains(feed_id));
        if feeds.is_empty() {
            return;
        }

        let feed_ids: Vec<FeedId> = feeds.iter().map(|(id, _)| id).cloned().collect();
        self.updating_feeds.extend(feed_ids.iter().cloned());
        self.notify_update_listener(FeedUpdateNotification::UpdateStarted(feed_ids));

        for (feed_id, source) in feeds {
            let permit_fut = Arc::clone(&self.feeds_semaphore).acquire_owned();
            let future = wrap_future(async move {
                let _permit = permit_fut.await.unwrap();
                fetch_feed(&source).await
            })
            .map(move |result, library: &mut Library, _ctx| {
                library.updating_feeds.remove(&feed_id);
                let result: Result<_, QueryError> = match result {
                    Ok(mut feed) => (|| {
                        let mut writer = library.data_provider.writer(feed_id)?;
                        let feed_metadata = feed.feed_metadata();
                        let mut feed_summary =
                            FeedSummary::from_metadata(feed_id, &feed_metadata, 0);
                        writer.set_feed_metadata(&feed_metadata)?;
                        while let Some(episode_metadata) = feed.next_episode_metadata() {
                            if episode_metadata.block {
                                writer.delete_episode(episode_metadata.guid)?;
                            } else {
                                writer.set_episode_metadata(&episode_metadata)?;
                            }
                        }
                        writer.close()?;

                        let new_episodes_query = EpisodesQuery::default()
                            .feed_id(feed_id)
                            .status(EpisodeSummaryStatus::New);
                        feed_summary.new_count =
                            library.data_provider.count_episodes(new_episodes_query)?;

                        let feed = library.data_provider.get_feed(feed_id)?;
                        if let Some((overriden, title)) = feed.and_then(|feed| {
                            let overridden = feed.title_overriden;
                            feed.title.map(|title| (overridden, title))
                        }) {
                            if overriden {
                                feed_summary.title = title;
                            }
                        }

                        library.notify_update_listener(FeedUpdateNotification::UpdateFinished(
                            feed_id,
                            FeedUpdateResult::Updated(feed_summary),
                        ));
                        Ok(())
                    })(),
                    Err(err) => {
                        log::error!(target: "networking", "{}", err);
                        let new_status = FeedStatus::Error(err.as_feed_error());
                        if let Err(error) =
                            library.data_provider.set_feed_status(feed_id, new_status)
                        {
                            log::error!(target: "sql", "cannot update, {}", error);
                        }
                        library.notify_update_listener(FeedUpdateNotification::UpdateFinished(
                            feed_id,
                            FeedUpdateResult::StatusChanged(new_status),
                        ));
                        Ok(())
                    }
                };

                if let Err(error) = result {
                    log::error!(target: "sql", "cannot update, {}", error);
                };
            });
            ctx.spawn(future);
        }
    }
}

#[derive(Debug)]
pub enum FeedUpdateResult {
    Updated(FeedSummary),
    StatusChanged(FeedStatus),
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub enum FeedUpdateNotification {
    UpdateStarted(Vec<FeedId>),
    UpdateFinished(FeedId, FeedUpdateResult),
    FeedAdded(FeedSummary),
    FeedDeleted(FeedId),
    GroupAdded(GroupSummary),
    NewCountUpdated(HashMap<FeedId, usize>),
}

#[derive(Debug)]
pub enum UpdateQuery {
    Single(FeedId),
    All,
    Pending,
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub enum FeedUpdateRequest {
    Subscribe(Recipient<FeedUpdateNotification>),
    AddFeed(NewFeedMetadata),
    AddGroup(String),
    DeleteFeed(FeedId),
    DeleteGroup(GroupId),
    RenameFeed(FeedId, String),
    RenameGroup(GroupId, String),
    Update(UpdateQuery),
    SetGroup(GroupId, FeedId),
    AddArchive(FeedId, String),
    SetStatus(EpisodesQuery, EpisodeStatus),
    SetHidden(EpisodesQuery, bool),
    SetFeedEnabled(FeedId, bool),
    ReverseFeedOrder(FeedId),
}

impl Handler<FeedUpdateRequest> for Library {
    type Result = ();

    fn handle(&mut self, msg: FeedUpdateRequest, ctx: &mut Self::Context) -> Self::Result {
        match msg {
            FeedUpdateRequest::Subscribe(recipient) => self.update_listener = Some(recipient),
            FeedUpdateRequest::Update(query) => {
                match self.data_provider.get_update_sources(query) {
                    Ok(sources) => self.schedule_update(sources, ctx),
                    Err(error) => {
                        log::error!(target: "sql", "cannot update, {}", error);
                    }
                }
            }

            FeedUpdateRequest::AddArchive(feed_id, feed_url) => {
                self.schedule_update(vec![(feed_id, feed_url)], ctx);
            }
            FeedUpdateRequest::AddFeed(data) => {
                let feed_id = match self.data_provider.create_feed_pending(&data) {
                    Ok(Some(feed_id)) => feed_id,
                    Ok(None) => {
                        log::warn!("This podcast has already been added");
                        return;
                    }
                    Err(error) => {
                        log::error!(target: "sql", "cannot add feed, {}", error);
                        return;
                    }
                };

                let source = data.source.clone();
                self.notify_update_listener(FeedUpdateNotification::FeedAdded(
                    FeedSummary::new_created(feed_id, data),
                ));
                self.schedule_update(vec![(feed_id, source)], ctx);
            }
            FeedUpdateRequest::AddGroup(name) => match self.data_provider.create_group(&name) {
                Ok(Some(group_id)) => {
                    let summary = GroupSummary { id: group_id, name };
                    self.notify_update_listener(FeedUpdateNotification::GroupAdded(summary));
                }
                Ok(None) => {
                    log::warn!("The group with this name already exists");
                }
                Err(error) => {
                    log::error!(target: "sql", "cannot create group, {}", error);
                }
            },
            FeedUpdateRequest::DeleteFeed(feed_id) => {
                match self.data_provider.delete_feed(feed_id) {
                    Ok(_) => {
                        self.notify_update_listener(FeedUpdateNotification::FeedDeleted(feed_id));
                    }
                    Err(error) => {
                        log::error!(target: "sql", "cannot delete feed, {}", error);
                    }
                }
            }
            FeedUpdateRequest::DeleteGroup(group_id) => {
                if let Err(error) = self.data_provider.delete_group(group_id) {
                    log::error!(target: "sql", "cannot delete group, {}", error);
                }
            }
            FeedUpdateRequest::RenameFeed(feed_id, name) => {
                if let Err(error) = self.data_provider.rename_feed(feed_id, name) {
                    log::error!(target: "sql", "cannot rename feed, {}", error);
                }
            }
            FeedUpdateRequest::RenameGroup(group_id, name) => {
                if let Err(error) = self.data_provider.rename_group(group_id, name) {
                    log::error!(target: "sql", "cannot rename group, {}", error);
                }
            }
            FeedUpdateRequest::SetStatus(query, status) => {
                let result: Result<(), QueryError> = (|| {
                    let updated_feeds = self.data_provider.set_episode_status(query, status)?;
                    let new_episodes_count =
                        self.data_provider.get_new_episodes_count(updated_feeds)?;
                    self.notify_update_listener(FeedUpdateNotification::NewCountUpdated(
                        new_episodes_count,
                    ));
                    Ok(())
                })();
                if let Err(error) = result {
                    log::error!(target: "sql", "cannot update status, {}", error);
                }
            }
            FeedUpdateRequest::SetHidden(query, hidden) => {
                if let Err(error) = self.data_provider.set_episode_hidden(query, hidden) {
                    log::error!(target: "sql", "cannot update hidden flag, {}", error);
                }
            }
            FeedUpdateRequest::SetFeedEnabled(feed_id, enabled) => {
                if let Err(error) = self.data_provider.set_feed_enabled(feed_id, enabled) {
                    log::error!(target: "sql", "cannot update enabled flag, {}", error);
                }
            }
            FeedUpdateRequest::ReverseFeedOrder(feed_id) => {
                if let Err(error) = self.data_provider.reverse_feed_order(feed_id) {
                    log::error!(target: "sql", "cannot reverse order, {}", error);
                }
            }
            FeedUpdateRequest::SetGroup(group_id, feed_id) => {
                if let Err(error) = self.data_provider.add_feed_to_group(group_id, feed_id) {
                    log::error!(target: "sql", "cannot assign group, {}", error);
                }
            }
        }
    }
}
