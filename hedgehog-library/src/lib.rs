mod actor;
pub mod datasource;
pub mod metadata;
pub mod model;
mod rss_client;
mod sqlite;

pub use rss;

pub use actor::Library;
pub use sqlite::SqliteDataProvider;

pub use actor::{
    EpisodePlaybackDataRequest, EpisodeSummariesRequest, EpisodesCountRequest,
    FeedSummariesRequest, FeedUpdateNotification, FeedUpdateRequest, FeedUpdateResult,
};
pub use datasource::{EpisodesQuery, Page};
