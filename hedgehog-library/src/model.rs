use crate::metadata::FeedMetadata;
use chrono::{DateTime, Utc};
use cmd_parser::CmdParsable;
use rusqlite::types::{FromSql, ToSql};
use std::time::Duration;

macro_rules! entity_id {
    ($name:ident) => {
        #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
        pub struct $name(pub i64);

        impl FromSql for $name {
            fn column_result(
                value: rusqlite::types::ValueRef<'_>,
            ) -> rusqlite::types::FromSqlResult<Self> {
                FromSql::column_result(value).map($name)
            }
        }

        impl ToSql for $name {
            fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
                ToSql::to_sql(&self.0)
            }
        }
    };
}

entity_id!(FeedId);
entity_id!(EpisodeId);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedError {
    MalformedFeed,
    NetworkingError,
    HttpError(reqwest::StatusCode),
    Unknown,
}

impl FeedError {
    const HTTP_ERROR_MASK: u32 = 0x0001_0000;

    pub(crate) fn from_u32(value: u32) -> FeedError {
        match value {
            1 => FeedError::MalformedFeed,
            2 => FeedError::NetworkingError,
            value if value & Self::HTTP_ERROR_MASK != 0 => {
                match reqwest::StatusCode::from_u16((value & 0xFFFF) as u16) {
                    Ok(status_code) => FeedError::HttpError(status_code),
                    Err(_) => FeedError::Unknown,
                }
            }
            _ => FeedError::Unknown,
        }
    }

    pub(crate) fn as_u32(&self) -> u32 {
        match self {
            FeedError::MalformedFeed => 1,
            FeedError::NetworkingError => 2,
            FeedError::HttpError(status_code) => {
                status_code.as_u16() as u32 | Self::HTTP_ERROR_MASK
            }
            FeedError::Unknown => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedStatus {
    Pending,
    Loaded,
    Error(FeedError),
}

impl FeedStatus {
    pub(crate) fn db_view(&self) -> (u32, u32) {
        match self {
            FeedStatus::Pending => (0, 0),
            FeedStatus::Loaded => (1, 0),
            FeedStatus::Error(error) => (2, error.as_u32()),
        }
    }

    pub(crate) fn from_db(status: u32, error: u32) -> Self {
        match (status, error) {
            (1, _) => FeedStatus::Loaded,
            (2, error) => FeedStatus::Error(FeedError::from_u32(error)),
            (_, _) => FeedStatus::Pending,
        }
    }
}

pub trait Identifiable {
    type Id: Eq;
    fn id(&self) -> Self::Id;
}

#[derive(Debug)]
pub struct FeedSummary {
    pub id: FeedId,
    pub title: String,
    pub has_title: bool,
    pub status: FeedStatus,
}

impl FeedSummary {
    pub(crate) fn new_created(id: FeedId, source: String) -> Self {
        FeedSummary {
            id,
            title: source,
            has_title: false,
            status: FeedStatus::Pending,
        }
    }

    pub(crate) fn from_metadata(feed_id: FeedId, metadata: &FeedMetadata) -> Self {
        FeedSummary {
            id: feed_id,
            title: metadata.title.to_string(),
            has_title: true,
            status: FeedStatus::Loaded,
        }
    }
}

impl Identifiable for FeedSummary {
    type Id = FeedId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

pub struct Feed {
    pub id: FeedId,
    pub title: Option<String>,
    pub description: Option<String>,
    pub link: Option<String>,
    pub author: Option<String>,
    pub copyright: Option<String>,
    pub source: String,
    pub status: FeedStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, CmdParsable)]
pub enum EpisodeStatus {
    New,
    #[cmd(rename = "seen")]
    NotStarted,
    #[cmd(rename = "done")]
    Finished,
    #[cmd(ignore)]
    Started(Duration),
    #[cmd(ignore)]
    Error(Duration),
}

impl EpisodeStatus {
    pub(crate) fn from_db(status: usize, position: Duration) -> Self {
        match status {
            1 => EpisodeStatus::NotStarted,
            2 => EpisodeStatus::Finished,
            3 => EpisodeStatus::Started(position),
            4 => EpisodeStatus::Error(position),
            _ => EpisodeStatus::New,
        }
    }

    pub(crate) fn db_view(&self) -> (usize, Duration) {
        match self {
            EpisodeStatus::New => (0, Duration::ZERO),
            EpisodeStatus::NotStarted => (1, Duration::ZERO),
            EpisodeStatus::Finished => (2, Duration::ZERO),
            EpisodeStatus::Started(position) => (3, *position),
            EpisodeStatus::Error(position) => (4, *position),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EpisodeSummaryStatus {
    New,
    NotStarted,
    Finished,
    Started,
    Error,
}

impl From<EpisodeStatus> for EpisodeSummaryStatus {
    fn from(status: EpisodeStatus) -> Self {
        match status {
            EpisodeStatus::New => EpisodeSummaryStatus::New,
            EpisodeStatus::NotStarted => EpisodeSummaryStatus::NotStarted,
            EpisodeStatus::Finished => EpisodeSummaryStatus::Finished,
            EpisodeStatus::Started(_) => EpisodeSummaryStatus::Started,
            EpisodeStatus::Error(_) => EpisodeSummaryStatus::Error,
        }
    }
}

impl EpisodeSummaryStatus {
    pub(crate) fn from_db(status: usize) -> Self {
        match status {
            1 => EpisodeSummaryStatus::NotStarted,
            2 => EpisodeSummaryStatus::Finished,
            3 => EpisodeSummaryStatus::Started,
            4 => EpisodeSummaryStatus::Error,
            _ => EpisodeSummaryStatus::New,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct EpisodeSummary {
    pub id: EpisodeId,
    pub feed_id: FeedId,
    pub episode_number: Option<i64>,
    pub season_number: Option<i64>,
    pub title: Option<String>,
    pub status: EpisodeSummaryStatus,
    pub duration: Option<Duration>,
    pub publication_date: Option<DateTime<Utc>>,
}

impl Identifiable for EpisodeSummary {
    type Id = EpisodeId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

pub struct Episode {
    pub id: EpisodeId,
    pub feed_id: FeedId,
    pub episode_number: Option<i64>,
    pub season_number: Option<i64>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub link: Option<String>,
    pub status: EpisodeStatus,
    pub duration: Option<Duration>,
    pub publication_date: Option<DateTime<Utc>>,
    pub media_url: String,
}

pub struct EpisodePlaybackData {
    pub id: EpisodeId,
    pub media_url: String,
    pub position: Duration,
    pub duration: Option<Duration>,
}

#[derive(Debug, Default)]
pub struct EpisodesListMetadata {
    pub items_count: usize,
    pub max_season_number: Option<i64>,
    pub max_episode_number: Option<i64>,
    pub max_duration: Option<Duration>,
    pub has_publication_date: bool,
}
