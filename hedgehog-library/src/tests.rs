#![cfg(test)]

use crate::model::{FeedError, FeedStatus};
use crate::sqlite::SqliteDataProvider;
use crate::{
    FeedSummariesRequest, FeedUpdateNotification, FeedUpdateRequest, FeedUpdateResult, Library,
};
use actix::prelude::*;
use reqwest::StatusCode;
use tokio::sync::mpsc::{channel, Sender};

struct NotificationListener {
    messages: Sender<FeedUpdateNotification>,
}

impl NotificationListener {
    fn new(sender: Sender<FeedUpdateNotification>) -> Self {
        NotificationListener { messages: sender }
    }
}

impl Actor for NotificationListener {
    type Context = Context<Self>;
}

impl Handler<FeedUpdateNotification> for NotificationListener {
    type Result = ResponseFuture<()>;

    fn handle(&mut self, msg: FeedUpdateNotification, _ctx: &mut Self::Context) -> Self::Result {
        let sender = self.messages.clone();
        Box::pin(async move {
            sender.send(msg).await.unwrap();
        })
    }
}

macro_rules! let_assert {
    (let $first:ident$(::$tail:ident)* ($($var:ident),*) = $value:expr) => {
       let ($($var,)*) = match $value {
           $first$(::$tail)* ($($var),*) => ($($var,)*),
           msg => panic!("wrong variant: {:?}", msg),
       };
    };
}

#[actix::test]
async fn adding_new_feed() {
    let mock_server = httpmock::MockServer::start();
    let _mock = mock_server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/feed.xml");
        then.status(200)
            .header("content-type", "application/xml")
            .body(include_str!("./test_data/bedtime.xml"));
    });

    let provider = SqliteDataProvider::connect(":memory:").unwrap();
    let library = Library::new(provider).start();
    let (sender, mut reciever) = channel(16);
    let notifications = NotificationListener::new(sender).start();
    let msg = FeedUpdateRequest::Subscribe(notifications.recipient());
    library.send(msg).await.unwrap();

    let summaries = library.send(FeedSummariesRequest).await.unwrap().unwrap();
    assert_eq!(summaries.len(), 0);

    let source_url = format!("{}/feed.xml", mock_server.base_url());
    let msg = FeedUpdateRequest::AddFeed(source_url.clone());
    library.send(msg).await.unwrap();

    let feed_added = reciever.recv().await.unwrap();
    let_assert!(let FeedUpdateNotification::FeedAdded(summary) = feed_added);
    assert_eq!(summary.title, source_url);
    assert!(!summary.has_title);
    assert_eq!(summary.status, FeedStatus::Pending);

    let feed_update_started = reciever.recv().await.unwrap();
    let_assert!(let FeedUpdateNotification::UpdateStarted(feed_ids) = feed_update_started);
    assert_eq!(feed_ids, vec![summary.id]);

    let update_finished = reciever.recv().await.unwrap();
    let_assert!(let FeedUpdateNotification::UpdateFinished(feed_id, update_result) = update_finished);
    assert_eq!(feed_id, summary.id);
    let_assert!(let FeedUpdateResult::Updated(updated_summary) = update_result);
    assert_eq!(updated_summary.id, summary.id);
    assert_eq!(updated_summary.title, "Bedtime in the Public Domain");
    assert!(updated_summary.has_title);
    assert_eq!(updated_summary.status, FeedStatus::Loaded);

    let summaries = library.send(FeedSummariesRequest).await.unwrap().unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0], updated_summary);
}

#[actix::test]
async fn adding_new_feed_error() {
    let mock_server = httpmock::MockServer::start();
    let _mock = mock_server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/feed.xml");
        then.status(404);
    });

    let provider = SqliteDataProvider::connect(":memory:").unwrap();
    let library = Library::new(provider).start();
    let (sender, mut reciever) = channel(16);
    let notifications = NotificationListener::new(sender).start();
    let msg = FeedUpdateRequest::Subscribe(notifications.recipient());
    library.send(msg).await.unwrap();

    let source_url = format!("{}/feed.xml", mock_server.base_url());
    let msg = FeedUpdateRequest::AddFeed(source_url.clone());
    library.send(msg).await.unwrap();

    let feed_added = reciever.recv().await.unwrap();
    let_assert!(let FeedUpdateNotification::FeedAdded(summary) = feed_added);

    let feed_update_started = reciever.recv().await.unwrap();
    let_assert!(let FeedUpdateNotification::UpdateStarted(_feed_ids) = feed_update_started);

    let update_finished = reciever.recv().await.unwrap();
    let_assert!(let FeedUpdateNotification::UpdateFinished(feed_id, update_result) = update_finished);
    assert_eq!(feed_id, summary.id);
    let_assert!(let FeedUpdateResult::StatusChanged(status) = update_result);
    let_assert!(let FeedStatus::Error(error) = status);
    assert_eq!(
        error,
        FeedError::HttpError(StatusCode::from_u16(404).unwrap())
    );

    let summaries = library.send(FeedSummariesRequest).await.unwrap().unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, summary.id);
    assert_eq!(summaries[0].title, source_url);
    assert!(!summaries[0].has_title);
    assert_eq!(summaries[0].status, status);
}
