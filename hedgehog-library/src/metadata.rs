use chrono::{DateTime, Utc};
use std::time::Duration;

#[derive(Debug, PartialEq)]
pub struct FeedMetadata<'a> {
    pub(crate) title: &'a str,
    pub(crate) description: &'a str,
    pub(crate) link: &'a str,
    pub(crate) author: Option<&'a str>,
    pub(crate) copyright: Option<&'a str>,
}

impl<'a> FeedMetadata<'a> {
    pub fn from_rss_channel(channel: &'a rss::Channel) -> Self {
        FeedMetadata {
            title: &channel.title,
            description: &channel.description,
            link: &channel.link,
            author: channel
                .itunes_ext
                .as_ref()
                .and_then(|ext| ext.author.as_deref()),
            copyright: channel.copyright.as_deref(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct EpisodeMetadata<'a> {
    pub(crate) title: Option<&'a str>,
    pub(crate) description: Option<&'a str>,
    pub(crate) link: Option<&'a str>,
    pub(crate) guid: &'a str,
    pub(crate) duration: Option<Duration>,
    pub(crate) publication_date: Option<DateTime<Utc>>,
    pub(crate) episode_number: Option<i64>,
    pub(crate) season_number: Option<i64>,
    pub(crate) media_url: &'a str,
}

impl<'a> EpisodeMetadata<'a> {
    pub fn from_rss_item(item: &'a rss::Item) -> Option<Self> {
        let publication_date = item
            .pub_date
            .as_deref()
            .map(|datetime| DateTime::parse_from_rfc2822(datetime))
            .transpose()
            .ok()?
            .map(|datetime| datetime.with_timezone(&Utc));
        let media_url = item.enclosure.as_ref().map(|enclosure| &enclosure.url)?;
        let guid = item
            .guid
            .as_ref()
            .map(|guid| &guid.value)
            .unwrap_or_else(|| media_url);
        let duration = item
            .itunes_ext
            .as_ref()
            .and_then(|ext| ext.duration.as_ref())
            .and_then(|duration| parse_itunes_duration(duration));
        let episode_number = item
            .itunes_ext
            .as_ref()
            .and_then(|ext| ext.episode.as_ref())
            .and_then(|episode| episode.parse().ok());
        let season_number = item
            .itunes_ext
            .as_ref()
            .and_then(|ext| ext.season.as_ref())
            .and_then(|season| season.parse().ok());

        Some(Self {
            title: item.title.as_deref(),
            description: item.description.as_deref(),
            link: item.link.as_deref(),
            guid,
            duration,
            publication_date,
            episode_number,
            media_url,
            season_number,
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
    use super::{EpisodeMetadata, FeedMetadata};
    use chrono::TimeZone;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use std::time::Duration;

    #[test]
    fn feed_from_channel() {
        let channel = rss::Channel {
            title: "Feed title".to_string(),
            description: "Feed description".to_string(),
            link: "http://example.com/feed".to_string(),
            copyright: Some("(c) Copyright".to_string()),
            itunes_ext: Some(rss::extension::itunes::ITunesChannelExtension {
                author: Some("Author".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let feed = FeedMetadata::from_rss_channel(&channel);
        assert_eq!(
            feed,
            FeedMetadata {
                title: "Feed title",
                description: "Feed description",
                link: "http://example.com/feed",
                author: Some("Author"),
                copyright: Some("(c) Copyright"),
            }
        );
    }

    #[test]
    fn episode_from_full() {
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
            itunes_ext: Some(rss::extension::itunes::ITunesItemExtension {
                duration: Some("30:00".to_string()),
                episode: Some("4".to_string()),
                season: Some("2".to_string()),
                ..Default::default()
            }),
            dublin_core_ext: None,
        };

        let episode = EpisodeMetadata::from_rss_item(&item).unwrap();
        assert_eq!(
            episode,
            EpisodeMetadata {
                title: Some("Episode title"),
                description: Some("Episode description"),
                link: Some("https://example.com/"),
                guid: "episode-guid",
                duration: Some(Duration::from_secs(1800)),
                publication_date: Some(chrono::Utc.ymd(2021, 9, 1).and_hms(14, 30, 0)),
                episode_number: Some(4),
                season_number: Some(2),
                media_url: "http://example.com/episode.mp3",
            }
        );
    }

    #[test]
    fn episode_from_mimimal() {
        let item = rss::Item {
            enclosure: Some(rss::Enclosure {
                url: "http://example.com/episode.mp3".to_string(),
                length: "1000".to_string(),
                mime_type: "audio/mpeg".to_string(),
            }),
            ..Default::default()
        };

        let episode = EpisodeMetadata::from_rss_item(&item).unwrap();
        assert_eq!(
            episode,
            EpisodeMetadata {
                title: None,
                description: None,
                link: None,
                guid: "http://example.com/episode.mp3",
                duration: None,
                publication_date: None,
                episode_number: None,
                season_number: None,
                media_url: "http://example.com/episode.mp3",
            }
        );
    }

    #[test]
    fn missing_enclosure() {
        let item = rss::Item::default();
        let result = EpisodeMetadata::from_rss_item(&item);
        assert!(result.is_none());
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
        assert_eq!(super::parse_itunes_duration("abc"), None);
    }

    #[test]
    fn time_from_invalid_many_components() {
        assert_eq!(super::parse_itunes_duration("10:20:30:40"), None);
    }
}
