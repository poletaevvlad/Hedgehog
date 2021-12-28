#![cfg(test)]

mod data;

use crate::model::{EpisodeSummary, FeedError, FeedId, FeedStatus};
use crate::sqlite::SqliteDataProvider;
use crate::{
    EpisodeSummariesRequest, EpisodesListMetadataRequest, EpisodesQuery, FeedSummariesRequest,
    FeedUpdateNotification, FeedUpdateRequest, FeedUpdateResult, Library, NewFeedMetadata,
    UpdateQuery,
};
use actix::prelude::*;
use reqwest::StatusCode;
use std::collections::HashSet;
use tokio::sync::mpsc::{channel, Receiver, Sender};

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

async fn create_library() -> (Addr<Library>, Receiver<FeedUpdateNotification>) {
    let provider = SqliteDataProvider::connect(":memory:").unwrap();
    let library = Library::new(provider).start();
    let (sender, reciever) = channel(16);
    let notifications = NotificationListener::new(sender).start();
    let msg = FeedUpdateRequest::Subscribe(notifications.recipient());
    library.send(msg).await.unwrap();
    (library, reciever)
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
    mock_server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/feed.xml");
        then.status(200)
            .header("content-type", "application/xml")
            .body(include_str!("../test_data/rss/feed1.xml"));
    });
    let (library, mut reciever) = create_library().await;

    let summaries = library.send(FeedSummariesRequest).await.unwrap().unwrap();
    assert_eq!(summaries.len(), 0);

    let source_url = format!("{}/feed.xml", mock_server.base_url());
    let msg = FeedUpdateRequest::AddFeed(NewFeedMetadata::new(source_url.clone()));
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
    assert_eq!(updated_summary.title, "Sample Podcast");
    assert!(updated_summary.has_title);
    assert_eq!(updated_summary.status, FeedStatus::Loaded);

    let summaries = library.send(FeedSummariesRequest).await.unwrap().unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0], updated_summary);
}

#[actix::test]
async fn adding_new_feed_error() {
    let mock_server = httpmock::MockServer::start();
    mock_server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/feed.xml");
        then.status(404);
    });
    let (library, mut reciever) = create_library().await;

    let source_url = format!("{}/feed.xml", mock_server.base_url());
    let msg = FeedUpdateRequest::AddFeed(NewFeedMetadata::new(source_url.clone()));
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

async fn seed_feed(
    server: &httpmock::MockServer,
    library: Addr<Library>,
    reciever: &mut Receiver<FeedUpdateNotification>,
    xml: &str,
) -> FeedId {
    let mut mock = server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/feed.xml");
        then.status(200).body(xml);
    });

    let source_url = format!("{}/feed.xml", server.base_url());
    let msg = FeedUpdateRequest::AddFeed(NewFeedMetadata::new(source_url.clone()));

    library.send(msg).await.unwrap();

    loop {
        let msg = reciever.recv().await.unwrap();
        if let FeedUpdateNotification::UpdateFinished(feed_id, _) = msg {
            mock.delete();
            return feed_id;
        }
    }
}

async fn get_episode_summaries(
    library: Addr<Library>,
    query: EpisodesQuery,
) -> Vec<EpisodeSummary> {
    let list_metadata = library
        .send(EpisodesListMetadataRequest(query.clone()))
        .await
        .unwrap()
        .unwrap();

    let mut episodes = Vec::with_capacity(list_metadata.items_count);
    let mut offset = 0;
    while offset < list_metadata.items_count {
        let page = library
            .send(EpisodeSummariesRequest::new(
                query.clone(),
                offset..(offset + 2),
            ))
            .await
            .unwrap()
            .unwrap();
        assert!(page.len() <= 2);
        episodes.extend(page);
        offset += 2;
    }

    let empty_page = library
        .send(EpisodeSummariesRequest::new(
            query.clone(),
            offset..(offset + 2),
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(empty_page.is_empty());

    assert_eq!(
        list_metadata.max_season_number,
        episodes.iter().filter_map(|ep| ep.season_number).max(),
    );
    assert_eq!(
        list_metadata.max_episode_number,
        episodes.iter().filter_map(|ep| ep.episode_number).max(),
    );
    assert_eq!(
        list_metadata.max_duration,
        episodes.iter().filter_map(|ep| ep.duration).max(),
    );

    episodes
}

#[actix::test]
async fn creates_episodes() {
    let (library, mut reciever) = create_library().await;
    let feed = include_str!("../test_data/rss/feed1.xml");
    let mock_server = httpmock::MockServer::start();
    let feed_id = seed_feed(&mock_server, library.clone(), &mut reciever, feed).await;

    let query = EpisodesQuery::default()
        .feed_id(feed_id)
        .include_feed_title();
    let episodes = get_episode_summaries(library, query).await;
    assert!(episodes.iter().all(|ep| ep.feed_id == feed_id));
    let expected = [
        data::feed1::EPISODE_5,
        data::feed1::EPISODE_4,
        data::feed1::EPISODE_3,
        data::feed1::EPISODE_2,
        data::feed1::EPISODE_1,
    ];
    assert_eq!(episodes.len(), expected.len());
    for (expected, actual) in expected.iter().zip(episodes.iter()) {
        expected.assert_equals(actual);
    }
}

#[actix::test]
async fn updates_episodes_on_update() {
    let (library, mut reciever) = create_library().await;
    let feed = include_str!("../test_data/rss/feed1.xml");
    let mock_server = httpmock::MockServer::start();
    let feed_id = seed_feed(&mock_server, library.clone(), &mut reciever, feed).await;
    mock_server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/feed.xml");
        then.status(200)
            .body(include_str!("../test_data/rss/feed1-updated-episodes.xml"));
    });

    let msg = FeedUpdateRequest::Update(UpdateQuery::Single(feed_id));
    library.send(msg).await.unwrap();

    let update_started = reciever.recv().await.unwrap();
    let_assert!(let FeedUpdateNotification::UpdateStarted(ids) = update_started);
    assert_eq!(ids, vec![feed_id]);

    let update_finished = reciever.recv().await.unwrap();
    let_assert!(let FeedUpdateNotification::UpdateFinished(id, update) = update_finished);
    assert_eq!(id, feed_id);
    let_assert!(let FeedUpdateResult::Updated(summary) = update);
    assert_eq!(summary.id, feed_id);
    assert_eq!(&summary.title, "Sample Podcast");

    let query = EpisodesQuery::default()
        .feed_id(feed_id)
        .include_feed_title();
    let episodes = get_episode_summaries(library, query).await;
    assert!(episodes.iter().all(|ep| ep.feed_id == feed_id));
    let expected = [
        data::feed1::EPISODE_6,
        data::feed1::EPISODE_5,
        data::feed1::EPISODE_4,
        data::feed1::EPISODE_3_UPDATED,
        data::feed1::EPISODE_2,
        data::feed1::EPISODE_1,
    ];
    assert_eq!(episodes.len(), expected.len());
    for (expected, actual) in expected.iter().zip(episodes.iter()) {
        expected.assert_equals(actual);
    }
}

#[actix::test]
async fn removes_blocked_episodes() {
    let (library, mut reciever) = create_library().await;
    let feed = include_str!("../test_data/rss/feed1.xml");
    let mock_server = httpmock::MockServer::start();
    let feed_id = seed_feed(&mock_server, library.clone(), &mut reciever, feed).await;
    mock_server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/feed.xml");
        then.status(200)
            .body(include_str!("../test_data/rss/feed1-blocked-episode.xml"));
    });

    let msg = FeedUpdateRequest::Update(UpdateQuery::Single(feed_id));
    library.send(msg).await.unwrap();

    let update_started = reciever.recv().await.unwrap();
    let_assert!(let FeedUpdateNotification::UpdateStarted(_ids) = update_started);
    let update_finished = reciever.recv().await.unwrap();
    let_assert!(let FeedUpdateNotification::UpdateFinished(_id, update) = update_finished);
    let_assert!(let FeedUpdateResult::Updated(_summary) = update);

    let query = EpisodesQuery::default()
        .feed_id(feed_id)
        .include_feed_title();
    let episodes = get_episode_summaries(library, query).await;
    assert!(episodes.iter().all(|ep| ep.feed_id == feed_id));
    let expected = [
        data::feed1::EPISODE_5,
        data::feed1::EPISODE_4,
        data::feed1::EPISODE_3,
        data::feed1::EPISODE_1,
    ];
    assert_eq!(episodes.len(), expected.len());
    for (expected, actual) in expected.iter().zip(episodes.iter()) {
        expected.assert_equals(actual);
    }
}

#[actix::test]
async fn update_failure() {
    let (library, mut reciever) = create_library().await;
    let feed = include_str!("../test_data/rss/feed1.xml");
    let mock_server = httpmock::MockServer::start();
    let feed_id = seed_feed(&mock_server, library.clone(), &mut reciever, feed).await;
    mock_server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/feed.xml");
        then.status(500);
    });

    let msg = FeedUpdateRequest::Update(UpdateQuery::Single(feed_id));
    library.send(msg).await.unwrap();

    let _update_started = reciever.recv().await.unwrap();
    let update_finished = reciever.recv().await.unwrap();
    let_assert!(let FeedUpdateNotification::UpdateFinished(id, update) = update_finished);
    assert_eq!(id, feed_id);
    let_assert!(let FeedUpdateResult::StatusChanged(new_status) = update);
    assert_eq!(
        new_status,
        FeedStatus::Error(FeedError::HttpError(StatusCode::from_u16(500).unwrap()))
    );

    let query = EpisodesQuery::default()
        .feed_id(feed_id)
        .include_feed_title();
    let episodes = get_episode_summaries(library, query).await;
    assert!(episodes.iter().all(|ep| ep.feed_id == feed_id));
    let expected = [
        data::feed1::EPISODE_5,
        data::feed1::EPISODE_4,
        data::feed1::EPISODE_3,
        data::feed1::EPISODE_2,
        data::feed1::EPISODE_1,
    ];
    assert_eq!(episodes.len(), expected.len());
    for (expected, actual) in expected.iter().zip(episodes.iter()) {
        expected.assert_equals(actual);
    }
}

#[actix::test]
async fn update_all() {
    let (library, mut reciever) = create_library().await;
    let mock_server = httpmock::MockServer::start();

    let mut feed_ids = Vec::new();
    for i in 0..4 {
        mock_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/feed{}.xml", i));
            then.status(200)
                .body(include_str!("../test_data/rss/empty-feed.xml"));
        });

        let source = format!("{}/feed{}.xml", mock_server.base_url(), i);
        library
            .send(FeedUpdateRequest::AddFeed(NewFeedMetadata::new(source)))
            .await
            .unwrap();

        let mut feed_id = None;
        loop {
            match reciever.recv().await.unwrap() {
                FeedUpdateNotification::UpdateStarted(ids) => {
                    assert_eq!(ids, vec![feed_id.unwrap()]);
                }
                FeedUpdateNotification::UpdateFinished(id, _) => {
                    assert_eq!(id, feed_id.unwrap());
                    break;
                }
                FeedUpdateNotification::FeedAdded(summary) => feed_id = Some(summary.id),
                msg => panic!("unexpected message: {:?}", msg),
            }
        }
        feed_ids.push(feed_id.unwrap());
    }

    async fn get_updated(
        library: &Addr<Library>,
        reciever: &mut Receiver<FeedUpdateNotification>,
    ) -> HashSet<FeedId> {
        library
            .send(FeedUpdateRequest::Update(UpdateQuery::All))
            .await
            .unwrap();
        let update_started = reciever.recv().await.unwrap();
        let_assert!(let FeedUpdateNotification::UpdateStarted(feed_ids) = update_started);
        for _ in 0..feed_ids.len() {
            let update_started = reciever.recv().await.unwrap();
            let_assert!(let FeedUpdateNotification::UpdateFinished(id, _update) = update_started);
            assert!(feed_ids.contains(&id));
        }
        feed_ids.into_iter().collect()
    }

    let updated = get_updated(&library, &mut reciever).await;
    assert_eq!(updated, feed_ids.iter().cloned().collect());

    library
        .send(FeedUpdateRequest::SetFeedEnabled(feed_ids[1], false))
        .await
        .unwrap();
    library
        .send(FeedUpdateRequest::SetFeedEnabled(feed_ids[2], false))
        .await
        .unwrap();

    let updated = get_updated(&library, &mut reciever).await;
    assert_eq!(
        updated,
        vec![feed_ids[0], feed_ids[3]].iter().cloned().collect()
    );

    library
        .send(FeedUpdateRequest::SetFeedEnabled(feed_ids[0], true))
        .await
        .unwrap();
    library
        .send(FeedUpdateRequest::SetFeedEnabled(feed_ids[2], true))
        .await
        .unwrap();

    let updated = get_updated(&library, &mut reciever).await;
    assert_eq!(
        updated,
        vec![feed_ids[0], feed_ids[2], feed_ids[3]]
            .iter()
            .cloned()
            .collect()
    );
}
