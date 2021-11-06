use crate::datasource::{
    DataProvider, EpisodeWriter, ListQuery, PagedQueryHandler, QueryError, QueryHandler,
    WritableDataProvider,
};
use crate::model::{FeedId, FeedSummary};
use crate::rss_client::{fetch_feed, WritableFeed};
use crate::sqlite::SqliteDataProvider;
use actix::fut::wrap_future;
use actix::prelude::*;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Semaphore;

pub struct Library<D: DataProvider = SqliteDataProvider> {
    data_provider: D,
    updating_feeds: HashSet<FeedId>,
    feeds_semaphore: Arc<Semaphore>,
    update_listener: Option<Recipient<FeedUpdateNotification>>,
}

impl<D: DataProvider> Library<D> {
    pub fn new(data_provider: D) -> Self {
        Library {
            data_provider,
            updating_feeds: HashSet::new(),
            feeds_semaphore: Arc::new(Semaphore::new(8)),
            update_listener: None,
        }
    }
}

impl<D: DataProvider + 'static> Actor for Library<D> {
    type Context = Context<Self>;
}

#[derive(Message)]
#[rtype(result = "Result<Vec<Q::Item>, QueryError>")]
pub struct PagedQueryRequest<Q: ListQuery> {
    pub data: Q,
    pub count: usize,
    pub offset: usize,
}

impl<Q: ListQuery> PagedQueryRequest<Q> {
    pub fn new(data: Q, count: usize) -> Self {
        PagedQueryRequest {
            data,
            count,
            offset: 0,
        }
    }

    pub fn with_offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }
}

impl<D, Q> Handler<PagedQueryRequest<Q>> for Library<D>
where
    D: DataProvider + PagedQueryHandler<Q> + 'static,
    Q: ListQuery,
{
    type Result = Result<Vec<Q::Item>, QueryError>;

    fn handle(&mut self, msg: PagedQueryRequest<Q>, _ctx: &mut Self::Context) -> Self::Result {
        self.data_provider
            .query_page(msg.data, msg.offset, msg.count)
    }
}

#[derive(Message)]
#[rtype(result = "Result<usize, QueryError>")]
pub struct SizeRequest<Q: ListQuery>(pub Q);

impl<D, Q> Handler<SizeRequest<Q>> for Library<D>
where
    D: DataProvider + PagedQueryHandler<Q> + 'static,
    Q: ListQuery,
{
    type Result = Result<usize, QueryError>;

    fn handle(&mut self, msg: SizeRequest<Q>, _ctx: &mut Self::Context) -> Self::Result {
        self.data_provider.get_size(msg.0)
    }
}

#[derive(Message)]
#[rtype(result = "Result<Vec<Q::Item>, QueryError>")]
pub struct QueryRequest<Q: ListQuery>(pub Q);

impl<D, Q> Handler<QueryRequest<Q>> for Library<D>
where
    D: DataProvider + QueryHandler<Q> + 'static,
    Q: ListQuery,
{
    type Result = Result<Vec<Q::Item>, QueryError>;

    fn handle(&mut self, msg: QueryRequest<Q>, _ctx: &mut Self::Context) -> Self::Result {
        self.data_provider.query(msg.0)
    }
}

impl<D: DataProvider + 'static> Library<D> {
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
        mut feed_ids: Vec<FeedId>,
        ctx: &mut <Library<D> as Actor>::Context,
    ) where
        for<'a> &'a mut D: WritableDataProvider,
    {
        feed_ids.retain(|feed_id| !self.updating_feeds.contains(feed_id));
        if feed_ids.is_empty() {
            return;
        }

        self.updating_feeds.extend(feed_ids.iter().cloned());
        self.notify_update_listener(FeedUpdateNotification::UpdateStarted(feed_ids.clone()));

        for feed_id in feed_ids {
            let source = self.data_provider.get_feed_source(feed_id).unwrap();

            let permit_fut = Arc::clone(&self.feeds_semaphore).acquire_owned();
            let future = wrap_future(async move {
                let _permit = permit_fut.await.unwrap();
                fetch_feed(&source).await.map_err(FeedUpdateError::from)
            })
            .map(move |result, library: &mut Library<D>, _ctx| {
                library.updating_feeds.remove(&feed_id);
                let result = result.and_then(|mut feed| {
                    let mut writer = library.data_provider.writer(feed_id)?;
                    let feed_metadata = feed.feed_metadata();
                    let feed_summary = FeedSummary::from_metadata(feed_id, &feed_metadata);
                    writer.set_feed_metadata(&feed_metadata)?;
                    while let Some(episode_metadata) = feed.next_episode_metadata() {
                        writer.set_episode_metadata(&episode_metadata)?;
                    }
                    writer.close()?;
                    Ok(feed_summary)
                });

                match result {
                    Ok(feed_summary) => library.notify_update_listener(
                        FeedUpdateNotification::UpdateFinished(feed_id, feed_summary),
                    ),
                    Err(error) => {
                        // TODO: Update feed with error status
                        library.notify_update_listener(FeedUpdateNotification::Error(error))
                    }
                };
            });
            ctx.spawn(future);
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FeedUpdateError {
    #[error(transparent)]
    Database(#[from] QueryError),

    #[error(transparent)]
    Fetch(#[from] crate::rss_client::FetchError),
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub enum FeedUpdateNotification {
    UpdateStarted(Vec<FeedId>),
    UpdateFinished(FeedId, FeedSummary),
    Error(FeedUpdateError),
    FeedAdded(FeedSummary),
    FeedDeleted(FeedId),
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub enum FeedUpdateRequest {
    Subscribe(Recipient<FeedUpdateNotification>),
    AddFeed(String),
    DeleteFeed(FeedId),
    UpdateSingle(FeedId),
}

impl<D: DataProvider + 'static> Handler<FeedUpdateRequest> for Library<D>
where
    for<'a> &'a mut D: WritableDataProvider,
{
    type Result = ();

    fn handle(&mut self, msg: FeedUpdateRequest, ctx: &mut Self::Context) -> Self::Result {
        match msg {
            FeedUpdateRequest::Subscribe(recipient) => self.update_listener = Some(recipient),
            FeedUpdateRequest::UpdateSingle(feed_id) => self.schedule_update(vec![feed_id], ctx),
            FeedUpdateRequest::AddFeed(source) => {
                let feed_id = match self.data_provider.create_feed_pending(&source) {
                    Ok(feed_id) => feed_id,
                    Err(error) => {
                        self.notify_update_listener(FeedUpdateNotification::Error(error.into()));
                        return;
                    }
                };

                self.notify_update_listener(FeedUpdateNotification::FeedAdded(
                    FeedSummary::new_created(feed_id, source),
                ));
                self.schedule_update(vec![feed_id], ctx);
            }
            FeedUpdateRequest::DeleteFeed(feed_id) => {
                match self.data_provider.delete_feed(feed_id) {
                    Ok(_) => {
                        self.notify_update_listener(FeedUpdateNotification::FeedDeleted(feed_id))
                    }
                    Err(error) => {
                        self.notify_update_listener(FeedUpdateNotification::Error(error.into()))
                    }
                }
            }
        }
    }
}
