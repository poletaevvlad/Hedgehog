use super::model::{FeedStatus, FeedSummary};
use crate::metadata::FeedMetadata;
use crate::model::{Feed, FeedId};
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
    fn prepare(&self, statement: &str) -> Result<rusqlite::Statement, rusqlite::Error> {
        self.provider.connection.prepare(statement)
    }

    pub fn query_feeds(&self) -> Result<Vec<FeedSummary>, rusqlite::Error> {
        let mut select = self.prepare("SELECT id, title, source, status, error_code FROM feeds")?;
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

    pub fn get_feed(&self, id: FeedId) -> Result<Option<Feed>, rusqlite::Error> {
        let mut statement = self.prepare("SELECT id, title, description, link, author, copyright, source, status, error_code FROM feeds WHERE id = ?1")?;
        let result = statement.query_row([id], |row| {
            Ok(Feed {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                link: row.get(3)?,
                author: row.get(4)?,
                copyright: row.get(5)?,
                source: row.get(6)?,
                status: FeedStatus::from_db(row.get(7)?, row.get(8)?),
            })
        });
        match result {
            Ok(feed) => Ok(Some(feed)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub fn create_pending(&self, source: &str) -> Result<FeedId, rusqlite::Error> {
        let mut statement = self.prepare("INSERT INTO feeds (source) VALUES (?1)")?;
        statement.insert(params![source]).map(FeedId)
    }

    pub fn update_metadata(
        &self,
        id: FeedId,
        metadata: &FeedMetadata,
    ) -> Result<bool, rusqlite::Error> {
        let mut statement = self.prepare("UPDATE feeds SET title = ?1, description = ?2, link = ?3, author = ?4, copyright = ?5, status = ?6, error_code = ?7 WHERE id = ?8")?;
        let (status, error_code) = FeedStatus::Loaded.into_db();
        statement
            .execute(params![
                metadata.title,
                metadata.description,
                metadata.link,
                metadata.author,
                metadata.copyright,
                status,
                error_code,
                id
            ])
            .map(|updated| updated > 0)
    }

    pub fn update_status(&self, id: FeedId, status: FeedStatus) -> Result<bool, rusqlite::Error> {
        let mut statement =
            self.prepare("UPDATE feeds SET status = ?1, error_code = ?2 WHERE id = ?3")?;
        let (status, error_code) = status.into_db();
        statement
            .execute(params![status, error_code, id])
            .map(|updated| updated > 0)
    }

    pub fn delete(&self, id: FeedId) -> Result<bool, rusqlite::Error> {
        let mut statement = self.prepare("DELETE FROM feeds WHERE id = ?1")?;
        statement.execute([id]).map(|updated| updated > 0)
    }
}

#[cfg(test)]
mod tests {
    use crate::metadata::FeedMetadata;
    use crate::model::{FeedError, FeedStatus};

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
    fn crud_operations() {
        let provider = SqliteDataProvider::connect(":memory:").unwrap();
        let id = provider
            .feeds()
            .create_pending("http://example.com/feed.xml")
            .unwrap();

        let summaries = provider.feeds().query_feeds().unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, id);
        assert_eq!(summaries[0].title, None);
        assert_eq!(&summaries[0].source, "http://example.com/feed.xml");
        assert_eq!(summaries[0].status, FeedStatus::Pending);

        let is_updated = provider
            .feeds()
            .update_metadata(
                id,
                &FeedMetadata {
                    title: "Title".to_string(),
                    description: "Description".to_string(),
                    link: "http://example.com".to_string(),
                    author: Some("Author".to_string()),
                    copyright: Some("Copyright".to_string()),
                },
            )
            .unwrap();
        assert!(is_updated);

        let feed = provider.feeds().get_feed(id).unwrap().unwrap();
        assert_eq!(feed.title.as_deref(), Some("Title"));
        assert_eq!(feed.description.as_deref(), Some("Description"));
        assert_eq!(feed.link.as_deref(), Some("http://example.com"));
        assert_eq!(feed.author.as_deref(), Some("Author"));
        assert_eq!(feed.copyright.as_deref(), Some("Copyright"));
        assert_eq!(&feed.source, "http://example.com/feed.xml");
        assert_eq!(feed.status, FeedStatus::Loaded);

        let is_updated = provider
            .feeds()
            .update_status(id, FeedStatus::Error(FeedError::NotFound))
            .unwrap();
        assert!(is_updated);

        let summaries = provider.feeds().query_feeds().unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, id);
        assert_eq!(summaries[0].title.as_deref(), Some("Title"));
        assert_eq!(&summaries[0].source, "http://example.com/feed.xml");
        assert_eq!(summaries[0].status, FeedStatus::Error(FeedError::NotFound));

        let is_deleted = provider.feeds().delete(id).unwrap();
        assert!(is_deleted);
        let is_deleted = provider.feeds().delete(id).unwrap();
        assert!(!is_deleted);
    }
}
