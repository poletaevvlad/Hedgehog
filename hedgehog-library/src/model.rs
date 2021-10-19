use chrono::{DateTime, Utc};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use rusqlite::types::{FromSql, ToSql};

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone)]
pub struct EpisodeDuration(u64);

impl EpisodeDuration {
    pub fn from_seconds(seconds: u64) -> Self {
        EpisodeDuration(seconds * 1_000_000_000)
    }

    pub fn from_nanoseconds(nanoseconds: u64) -> Self {
        EpisodeDuration(nanoseconds)
    }
}

impl FromSql for EpisodeDuration {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        FromSql::column_result(value).map(EpisodeDuration)
    }
}

impl ToSql for EpisodeDuration {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive)]
pub enum FeedError {
    InvalidFeed = 1,
    NotFound = 2,
    Unknown = 0,
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
            FeedStatus::Error(error) => (2, *error as u32),
        }
    }

    pub(crate) fn from_db(status: u32, error: u32) -> Self {
        match (status, error) {
            (1, _) => FeedStatus::Loaded,
            (2, error) => {
                FeedStatus::Error(FeedError::from_u32(error).unwrap_or(FeedError::Unknown))
            }
            (_, _) => FeedStatus::Pending,
        }
    }
}

pub struct FeedSummary {
    pub id: FeedId,
    pub title: Option<String>,
    pub status: FeedStatus,
    pub source: String,
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
    Started(EpisodeDuration),
}

impl EpisodeStatus {
    pub(crate) fn from_db(is_finished: bool, position: Option<EpisodeDuration>) -> Self {
        match (is_finished, position) {
            (_, Some(position)) => EpisodeStatus::Started(position),
            (true, None) => EpisodeStatus::Finished,
            (false, None) => EpisodeStatus::NotStarted,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive)]
pub enum PlaybackError {
    NotFound = 1,
    FormatError = 2,
    Unknown = 0,
}

impl PlaybackError {
    pub(crate) fn from_db(value: u32) -> PlaybackError {
        PlaybackError::from_u32(value).unwrap_or(PlaybackError::Unknown)
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
    pub duration: Option<EpisodeDuration>,
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
    pub duration: Option<EpisodeDuration>,
    pub playback_error: Option<PlaybackError>,
    pub publication_date: Option<DateTime<Utc>>,
    pub media_url: String,
}
