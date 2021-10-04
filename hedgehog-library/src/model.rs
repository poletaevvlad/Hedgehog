use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use rusqlite::types::{FromSql, ToSql};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct FeedId(pub i64);

impl FromSql for FeedId {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        FromSql::column_result(value).map(FeedId)
    }
}

impl ToSql for FeedId {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        ToSql::to_sql(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive)]
pub enum FeedError {
    InvalidFeed = 1,
    NotFound = 2,
    Unknown = 0,
}

pub enum FeedStatus {
    NotProcessed,
    Loaded,
    Error(FeedError),
}

impl FeedStatus {
    pub(crate) fn into_db(&self) -> (u32, u32) {
        match self {
            FeedStatus::NotProcessed => (0, 0),
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
            (_, _) => FeedStatus::NotProcessed,
        }
    }
}

pub struct FeedSummary {
    pub id: FeedId,
    pub title: String,
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
