use chrono::{DateTime, Utc};
use std::convert::TryFrom;
use std::time::Duration;
use thiserror::Error;

pub struct FeedMetadata {
    title: String,
    description: String,
    link: String,
    author: Option<String>,
    copyright: Option<String>,
}

#[derive(Debug, PartialEq)]
pub struct EpisodeMetadata {
    title: Option<String>,
    description: Option<String>,
    link: Option<String>,
    guid: String,
    duration: Option<Duration>,
    publication_date: Option<DateTime<Utc>>,
    episode_number: Option<u64>,
    media_url: String,
}

#[derive(Debug, Error)]
pub enum NotPodcastError {
    #[error("the item is missing the enclosure")]
    MissingEnclosure,
    #[error("the item's pubDate is invalid")]
    InvalidDate(#[from] chrono::ParseError),
}

impl TryFrom<rss::Item> for EpisodeMetadata {
    type Error = NotPodcastError;

    fn try_from(item: rss::Item) -> Result<Self, Self::Error> {
        let publication_date = item
            .pub_date
            .map(|datetime| DateTime::parse_from_rfc2822(&datetime))
            .transpose()
            .map_err(NotPodcastError::InvalidDate)?
            .map(|datetime| datetime.with_timezone(&Utc));
        let media_url = item.enclosure.ok_or(NotPodcastError::MissingEnclosure)?.url;
        let guid = item
            .guid
            .map(|guid| guid.value)
            .unwrap_or_else(|| media_url.clone());
        let duration = item
            .itunes_ext
            .as_ref()
            .and_then(|ext| ext.duration.as_ref())
            .and_then(|duration| parse_itunes_duration(duration));
        let episode_number = item
            .itunes_ext
            .and_then(|ext| ext.episode)
            .and_then(|episode| episode.parse().ok());

        Ok(Self {
            title: item.title,
            description: item.description,
            link: item.link,
            guid,
            duration,
            publication_date,
            episode_number,
            media_url,
        })
    }
}

fn parse_itunes_duration(duration: &str) -> Option<Duration> {
    let mut seconds = 0;
    for part in duration.splitn(3, ':') {
        let part = part.parse::<u64>().ok()?;
        seconds = seconds * 60 + part;
    }
    Some(Duration::from_secs(seconds))
}

#[cfg(test)]
mod tests {
    use super::{EpisodeMetadata, NotPodcastError};
    use chrono::TimeZone;
    use std::collections::HashMap;
    use std::convert::TryInto;
    use std::time::Duration;

    #[test]
    fn episode_from_full() {
        let mut itunes_ext = rss::extension::itunes::ITunesItemExtension::default();
        itunes_ext.duration = Some("30:00".to_string());
        itunes_ext.episode = Some("4".to_string());
        let item = rss::Item {
            title: Some("Episode title".to_string()),
            link: Some("https://example.com/".to_string()),
            description: Some("Episode description".to_string()),
            author: Some("Author's name".to_string()),
            categories: vec![],
            comments: Some("Comments".to_string()),
            enclosure: Some(rss::Enclosure {
                url: "http://example.com/episode.mp3".to_string(),
                length: "1000".to_string(),
                mime_type: "audio/mpeg".to_string(),
            }),
            guid: Some(rss::Guid {
                value: "episode-guid".to_string(),
                permalink: false,
            }),
            pub_date: Some("Wed, 01 Sep 2021 14:30:00 GMT".to_string()),
            source: Some(rss::Source::default()),
            content: Some("content".to_string()),
            extensions: HashMap::new(),
            itunes_ext: Some(itunes_ext),
            dublin_core_ext: None,
        };

        let episode: EpisodeMetadata = item.try_into().unwrap();
        assert_eq!(
            episode,
            EpisodeMetadata {
                title: Some("Episode title".to_string()),
                description: Some("Episode description".to_string()),
                link: Some("https://example.com/".to_string()),
                guid: "episode-guid".to_string(),
                duration: Some(Duration::from_secs(1800)),
                publication_date: Some(chrono::Utc.ymd(2021, 9, 1).and_hms(14, 30, 0)),
                episode_number: Some(4),
                media_url: "http://example.com/episode.mp3".to_string(),
            }
        );
    }

    #[test]
    fn episode_from_mimimal() {
        let mut item = rss::Item::default();
        item.enclosure = Some(rss::Enclosure {
            url: "http://example.com/episode.mp3".to_string(),
            length: "1000".to_string(),
            mime_type: "audio/mpeg".to_string(),
        });

        let episode: EpisodeMetadata = item.try_into().unwrap();
        assert_eq!(
            episode,
            EpisodeMetadata {
                title: None,
                description: None,
                link: None,
                guid: "http://example.com/episode.mp3".to_string(),
                duration: None,
                publication_date: None,
                episode_number: None,
                media_url: "http://example.com/episode.mp3".to_string(),
            }
        );
    }

    #[test]
    fn missing_enclosure() {
        let item = rss::Item::default();
        let err = <rss::Item as TryInto<EpisodeMetadata>>::try_into(item).unwrap_err();
        assert!(matches!(err, NotPodcastError::MissingEnclosure));
    }

    #[test]
    fn time_from_seconds() {
        assert_eq!(
            super::parse_itunes_duration("125"),
            Some(Duration::from_secs(125))
        );
    }

    #[test]
    fn time_from_minutes_seconds() {
        assert_eq!(
            super::parse_itunes_duration("12:45"),
            Some(Duration::from_secs(12 * 60 + 45))
        );
    }

    #[test]
    fn time_from_hours_minutes_seconds() {
        assert_eq!(
            super::parse_itunes_duration("2:26:13"),
            Some(Duration::from_secs(2 * 3600 + 26 * 60 + 13))
        );
    }

    #[test]
    fn time_from_invalid() {
        assert_eq!(super::parse_itunes_duration("abc"), None)
    }

    #[test]
    fn time_from_invalid_many_components() {
        assert_eq!(super::parse_itunes_duration("10:20:30:40"), None)
    }
}
