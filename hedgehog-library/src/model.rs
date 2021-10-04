use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

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
    pub id: u64,
    pub title: String,
    pub status: FeedStatus,
    pub source: String,
}

pub struct Feed {
    pub id: u64,
    pub title: Option<String>,
    pub description: Option<String>,
    pub link: Option<String>,
    pub author: Option<String>,
    pub copyright: Option<String>,
    pub source: String,
    pub status: FeedStatus,
}
