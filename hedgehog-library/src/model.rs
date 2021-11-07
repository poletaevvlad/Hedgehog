use crate::metadata::FeedMetadata;
use chrono::{DateTime, Utc};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EpisodeStatus {
    NotStarted,
    Finished,
    Started(Duration),
}

impl EpisodeStatus {
    pub(crate) fn from_db(is_finished: bool, position: Option<Duration>) -> Self {
        match (is_finished, position) {
            (_, Some(position)) => EpisodeStatus::Started(position),
            (true, None) => EpisodeStatus::Finished,
            (false, None) => EpisodeStatus::NotStarted,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackError {
    NotFound,
    FormatError,
    Unknown,
}

impl PlaybackError {
    pub(crate) fn from_u32(value: u32) -> Self {
        match value {
            1 => PlaybackError::NotFound,
            2 => PlaybackError::FormatError,
            _ => PlaybackError::Unknown,
        }
    }

    fn as_u32(&self) -> u32 {
        match self {
            PlaybackError::NotFound => 1,
            PlaybackError::FormatError => 2,
            PlaybackError::Unknown => 0,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct EpisodeSummary {
    pub id: EpisodeId,
    pub feed_id: FeedId,
    pub episode_number: Option<u64>,
    pub title: Option<String>,
    pub is_new: bool,
    pub status: EpisodeStatus,
    pub duration: Option<Duration>,
    pub playback_error: Option<PlaybackError>,
    pub publication_date: Option<DateTime<Utc>>,
    pub media_url: String,
}

pub struct Episode {
    pub id: EpisodeId,
    pub feed_id: FeedId,
    pub episode_number: Option<u64>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub link: Option<String>,
    pub is_new: bool,
    pub status: EpisodeStatus,
    pub duration: Option<Duration>,
    pub playback_error: Option<PlaybackError>,
    pub publication_date: Option<DateTime<Utc>>,
    pub media_url: String,
}
