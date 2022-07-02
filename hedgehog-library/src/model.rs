use crate::{metadata::FeedMetadata, NewFeedMetadata};
use actix::MessageResponse;
use chrono::{DateTime, Utc};
use rusqlite::types::{FromSql, ToSql};
use std::fmt;
use std::time::Duration;

macro_rules! entity_id {
    ($name:ident) => {
        #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
        pub struct $name(pub i64);

        impl $name {
            pub fn as_i64(self) -> i64 {
                self.0
            }
        }

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
entity_id!(GroupId);

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

impl fmt::Display for FeedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FeedError::MalformedFeed => f.write_str("The feed is not a valid RSS. Please check the source URL."),
            FeedError::NetworkingError => f.write_str("Could not load the source URL. The problem may be with the remote server or with your internet connection."),
            FeedError::HttpError(code) => f.write_fmt(format_args!("The request to the server has failed (status code {}).", code)),
            FeedError::Unknown => f.write_str("An unknown error has occured."),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

pub struct GroupSummary {
    pub id: GroupId,
    pub name: String,
}

impl Identifiable for GroupSummary {
    type Id = GroupId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

#[derive(Debug, PartialEq)]
pub struct FeedSummary {
    pub id: FeedId,
    pub title: String,
    pub has_title: bool,
    pub status: FeedStatus,
    pub new_count: usize,
    pub group_id: Option<GroupId>,
}

impl FeedSummary {
    pub(crate) fn new_created(id: FeedId, data: NewFeedMetadata) -> Self {
        FeedSummary {
            id,
            has_title: data.title.is_some(),
            title: data.title.unwrap_or(data.source),
            status: FeedStatus::Pending,
            new_count: 0,
            group_id: None,
        }
    }

    pub(crate) fn from_metadata(
        feed_id: FeedId,
        metadata: &FeedMetadata,
        new_episodes_count: usize,
    ) -> Self {
        FeedSummary {
            id: feed_id,
            title: metadata.title.to_string(),
            has_title: true,
            status: FeedStatus::Loaded,
            new_count: new_episodes_count,
            group_id: None,
        }
    }
}

impl Identifiable for FeedSummary {
    type Id = FeedId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

pub struct FeedOMPLEntry {
    pub title: Option<String>,
    pub feed_source: String,
    pub link: Option<String>,
}

pub struct Feed {
    pub id: FeedId,
    pub title: Option<String>,
    pub title_overriden: bool,
    pub description: Option<String>,
    pub link: Option<String>,
    pub author: Option<String>,
    pub copyright: Option<String>,
    pub source: String,
    pub status: FeedStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, cmdparse::Parsable)]
pub enum EpisodeStatus {
    New,
    #[cmd(rename = "seen")]
    NotStarted,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, cmdparse::Parsable, Hash)]
pub enum EpisodeSummaryStatus {
    New,
    #[cmd(rename = "seen")]
    NotStarted,
    Finished,
    Started,
    Error,
}

impl From<&EpisodeStatus> for EpisodeSummaryStatus {
    fn from(status: &EpisodeStatus) -> Self {
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

    pub(crate) fn db_view(&self) -> usize {
        match self {
            EpisodeSummaryStatus::New => 0,
            EpisodeSummaryStatus::NotStarted => 1,
            EpisodeSummaryStatus::Finished => 2,
            EpisodeSummaryStatus::Started => 3,
            EpisodeSummaryStatus::Error => 4,
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
    pub feed_title: Option<String>,
    pub status: EpisodeSummaryStatus,
    pub duration: Option<Duration>,
    pub publication_date: Option<DateTime<Utc>>,
    pub is_hidden: bool,
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

#[derive(Debug, Clone)]
pub struct EpisodePlaybackData {
    pub id: EpisodeId,
    pub media_url: String,
    pub position: Duration,
    pub duration: Option<Duration>,
    pub episode_title: Option<String>,
    pub feed_id: FeedId,
    pub feed_title: Option<String>,
}

#[derive(Debug, Default, Clone, MessageResponse)]
pub struct EpisodesListMetadata {
    pub items_count: usize,
    pub max_season_number: Option<i64>,
    pub max_episode_number: Option<i64>,
    pub max_duration: Option<Duration>,
    pub has_publication_date: bool,
    pub reversed_order: bool,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum FeedView<F, G> {
    All,
    New,
    Feed(F),
    Group(G),
}

impl<F, G> FeedView<F, G> {
    pub fn as_feed(&self) -> Option<&F> {
        match self {
            FeedView::Feed(feed) => Some(feed),
            _ => None,
        }
    }

    pub fn as_feed_mut(&mut self) -> Option<&mut F> {
        match self {
            FeedView::Feed(feed) => Some(feed),
            _ => None,
        }
    }

    pub fn map_feed<R>(self, f: impl FnOnce(F) -> R) -> FeedView<R, G> {
        match self {
            FeedView::All => FeedView::All,
            FeedView::New => FeedView::New,
            FeedView::Feed(feed) => FeedView::Feed(f(feed)),
            FeedView::Group(group) => FeedView::Group(group),
        }
    }

    pub fn as_ref(&self) -> FeedView<&F, &G> {
        match self {
            FeedView::All => FeedView::All,
            FeedView::New => FeedView::New,
            FeedView::Feed(feed) => FeedView::Feed(feed),
            FeedView::Group(group) => FeedView::Group(group),
        }
    }
}

impl<F: Identifiable, G: Identifiable> Identifiable for FeedView<F, G> {
    type Id = FeedView<F::Id, G::Id>;

    fn id(&self) -> Self::Id {
        match self {
            FeedView::All => FeedView::All,
            FeedView::New => FeedView::New,
            FeedView::Feed(feed) => FeedView::Feed(feed.id()),
            FeedView::Group(group) => FeedView::Group(group.id()),
        }
    }
}
