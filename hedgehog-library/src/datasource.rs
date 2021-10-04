use crate::metadata::FeedMetadata;
use crate::model::{Feed, FeedId};

use super::model::{FeedStatus, FeedSummary};
use rusqlite::{params, Connection};
use std::path::Path;
use thiserror::Error;

fn collect_results<T, E>(items: impl IntoIterator<Item = Result<T, E>>) -> Result<Vec<T>, E> {
    let iter = items.into_iter();
    let mut result = Vec::with_capacity(iter.size_hint().0);
    for item in iter {
        result.push(item?);
    }
    Ok(result)
}

#[derive(Debug, Error)]
pub enum ConnectionError {
    #[error("Database query failed")]
    SqliteError(#[from] rusqlite::Error),
    #[error("Database was updated in a newer version of hedgehog (db version: {version}, current: {version})")]
    VersionUnknown { version: u32, current: u32 },
}

#[derive(Debug)]
pub struct SqliteDataProvider {
    connection: Connection,
}

impl SqliteDataProvider {
    const CURRENT_VERSION: u32 = 1;

    pub fn connect<P: AsRef<Path>>(path: P) -> Result<Self, ConnectionError> {
        let connection = Connection::open(path)?;
        let version = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > Self::CURRENT_VERSION {
            return Err(ConnectionError::VersionUnknown {
                version,
                current: Self::CURRENT_VERSION,
            });
        }

        if version < 1 {
            connection.execute_batch(include_str!("schema/init.sql"))?;
        }

        connection.pragma_update(None, "user_version", Self::CURRENT_VERSION)?;
        Ok(SqliteDataProvider { connection })
    }

    pub fn feeds(&self) -> FeedsDao {
        FeedsDao { provider: self }
    }
}

pub struct FeedsDao<'a> {
    provider: &'a SqliteDataProvider,
}

impl<'a> FeedsDao<'a> {
    pub fn query_feeds(&self) -> Result<Vec<FeedSummary>, rusqlite::Error> {
        let mut select = self
            .provider
            .connection
            .prepare("SELECT rowid, title, source, status, error_code FROM feeds")?;
        let rows = select.query_map([], |row| {
            Ok(FeedSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                source: row.get(2)?,
                status: FeedStatus::from_db(row.get(3)?, row.get(4)?),
            })
        })?;
        collect_results(rows)
    }

    pub fn get_feed(&self, _id: FeedId) -> Result<Option<Feed>, rusqlite::Error> {
        todo!();
    }

    pub fn create_pending(&self, source: &str) -> Result<FeedId, rusqlite::Error> {
        let mut statement = self
            .provider
            .connection
            .prepare("INSERT INTO feeds (source) VALUES (?1)")?;
        statement.insert(params![source]).map(FeedId)
    }

    pub fn update_metadata(
        &self,
        _id: FeedId,
        _metadata: FeedMetadata,
    ) -> Result<Option<bool>, rusqlite::Error> {
        todo!()
    }

    pub fn update_status(&self, _id: FeedId, _status: FeedStatus) -> Result<bool, rusqlite::Error> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use crate::model::FeedStatus;

    use super::{ConnectionError, SqliteDataProvider};

    #[test]
    fn initializes_if_new() {
        let dir = tempfile::tempdir().unwrap();
        let mut path = dir.path().to_path_buf();
        path.push("db.sqlite");

        SqliteDataProvider::connect(&path).unwrap();

        let connection = rusqlite::Connection::open(path).unwrap();
        let user_version: u32 = connection
            .pragma_query_value(None, "user_version", |value| value.get(0))
            .unwrap();
        assert_eq!(user_version, SqliteDataProvider::CURRENT_VERSION);
    }

    #[test]
    fn fails_if_newer() {
        let dir = tempfile::tempdir().unwrap();
        let mut path = dir.path().to_path_buf();
        path.push("db.sqlite");

        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .pragma_update(None, "user_version", 20u32)
            .unwrap();
        drop(connection);

        let error = SqliteDataProvider::connect(path).unwrap_err();
        assert!(matches!(
            error,
            ConnectionError::VersionUnknown {
                version: 20,
                current: 1
            }
        ));
    }

    #[test]
    fn inserting_pending() {
        let provider = SqliteDataProvider::connect(":memory:").unwrap();
        let id = provider
            .feeds()
            .create_pending("http://example.com")
            .unwrap();
        let summaries = provider.feeds().query_feeds().unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, id);
        assert_eq!(summaries[0].title, None);
        assert_eq!(&summaries[0].source, "http://example.com");
        assert_eq!(summaries[0].status, FeedStatus::Pending);
    }
}
