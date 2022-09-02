use crate::metadata::{EpisodeMetadata, FeedMetadata};
use crate::model::FeedError;
use std::io::{BufReader, Cursor};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("Networking error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Request failed: {0}")]
    FailedStatusCode(reqwest::StatusCode),

    #[error("Invalid format: {0}")]
    XmlError(#[from] rss::Error),
}

impl FetchError {
    pub(crate) fn as_feed_error(&self) -> FeedError {
        match self {
            FetchError::HttpError(_) => FeedError::NetworkingError,
            FetchError::FailedStatusCode(status_code) => FeedError::HttpError(*status_code),
            FetchError::XmlError(_) => FeedError::MalformedFeed,
        }
    }
}

pub(crate) async fn fetch_feed(url: &str) -> Result<impl WritableFeed + 'static, FetchError> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("Hedgehog ", env!("CARGO_PKG_VERSION")))
        .build()?;
    let request = client.get(url).timeout(Duration::from_secs(300));
    let response = request.send().await?;
    if !response.status().is_success() {
        return Err(FetchError::FailedStatusCode(response.status()));
    }

    let xml_text = response.bytes().await?;
    let channel = rss::Channel::read_from(BufReader::new(Cursor::new(xml_text)))?;
    Ok(XmlFeed {
        channel,
        item_index: 0,
    })
}

struct XmlFeed {
    channel: rss::Channel,
    item_index: usize,
}

pub(crate) trait WritableFeed {
    fn feed_metadata(&self) -> FeedMetadata;
    fn next_episode_metadata(&mut self) -> Option<EpisodeMetadata>;
}

impl WritableFeed for XmlFeed {
    fn feed_metadata(&self) -> FeedMetadata {
        FeedMetadata::from_rss_channel(&self.channel)
    }

    fn next_episode_metadata(&mut self) -> Option<EpisodeMetadata> {
        loop {
            let item = self.channel.items.get(self.item_index)?;
            self.item_index += 1;
            if let Some(episode) = EpisodeMetadata::from_rss_item(item) {
                return Some(episode);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{fetch_feed, WritableFeed};
    use httpmock::prelude::*;

    #[actix::test]
    async fn fetches_feed() {
        let mock_server = MockServer::start();
        let mock = mock_server.mock(|when, then| {
            when.method(GET).path("/podcast/feed.rss");
            then.status(200)
                .header("content-type", "text/xml")
                .body(include_str!("./test_data/rss/simple-feed.xml"));
        });

        let mut feed = fetch_feed(&mock_server.url("/podcast/feed.rss"))
            .await
            .unwrap();

        let feed_metadata = feed.feed_metadata();
        assert_eq!(feed_metadata.title, "Feed title");

        let episode_1 = feed.next_episode_metadata().unwrap();
        assert_eq!(episode_1.guid, "ep1");

        let episode_2 = feed.next_episode_metadata().unwrap();
        assert_eq!(episode_2.guid, "ep3");

        assert!(feed.next_episode_metadata().is_none());
        mock.assert();
    }
}
