use crate::datasource::{
    DataProvider, EpisodeSummariesQuery, FeedSummariesQuery, PagedQueryHandler, QueryError,
};
use crate::model::{
    Episode, EpisodeId, EpisodeStatus, EpisodeSummary, Feed, FeedId, FeedStatus, FeedSummary,
    PlaybackError,
};
use directories::BaseDirs;
use rusqlite::{named_params, Connection};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConnectionError {
    #[error("Database query failed")]
    SqliteError(#[from] rusqlite::Error),

    #[error("Database was updated in a newer version of hedgehog (db version: {version}, current: {version})")]
    VersionUnknown { version: u32, current: u32 },

    #[error("Directory for the database cannot be determined")]
    CannonDetermineDataDirectory,

    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error>),
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

    pub fn connect_default_path() -> Result<Self, ConnectionError> {
        let base_dirs = BaseDirs::new().ok_or(ConnectionError::CannonDetermineDataDirectory)?;
        let mut data_dir = base_dirs.data_dir().to_path_buf();
        data_dir.push("hedgehog");
        std::fs::create_dir_all(&data_dir)
            .map_err(|error| ConnectionError::Other(Box::new(error)))?;
        data_dir.push("episodes-db");
        Self::connect(data_dir)
    }
}

impl DataProvider for SqliteDataProvider {
    fn get_feed(&self, id: FeedId) -> Result<Option<crate::model::Feed>, QueryError> {
        let mut statement = self.connection.prepare("SELECT id, title, description, link, author, copyright, source, status, error_code FROM feeds WHERE id = ?1")?;
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
            Err(error) => Err(error.into()),
        }
    }

    fn create_feed_pending(&self, source: &str) -> Result<FeedId, QueryError> {
        let mut statement = self
            .connection
            .prepare("INSERT INTO feeds (source) VALUES (:id)")?;
        statement
            .insert(named_params! {":id": source})
            .map(FeedId)
            .map_err(Into::into)
    }

    fn get_episode(&self, episode_id: EpisodeId) -> Result<Option<Episode>, QueryError> {
        let mut statement =
            self.connection.prepare("SELECT feed_id, episode_number, title, description, link, is_new, is_finished, position, duration, error_code, publication_date, media_url FROM episodes WHERE id = :id")?;
        let result = statement.query_row(named_params! {":id": episode_id}, |row| {
            Ok(Episode {
                id: episode_id,
                feed_id: row.get(0)?,
                episode_number: row.get(1)?,
                title: row.get(2)?,
                description: row.get(3)?,
                link: row.get(4)?,
                is_new: row.get(5)?,
                status: EpisodeStatus::from_db(row.get(6)?, row.get(7)?),
                duration: row.get(8)?,
                playback_error: row.get::<_, Option<u32>>(9)?.map(PlaybackError::from_db),
                publication_date: row.get(10)?,
                media_url: row.get(11)?,
            })
        });
        match result {
            Ok(episode) => Ok(Some(episode)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

impl PagedQueryHandler<FeedSummariesQuery> for SqliteDataProvider {
    fn get_size(&self, _request: FeedSummariesQuery) -> Result<usize, QueryError> {
        let mut select = self.connection.prepare("SELECT COUNT(id) FROM feeds")?;
        Ok(select.query_row([], |row| row.get(0))?)
    }

    fn query(
        &self,
        _request: FeedSummariesQuery,
        offset: usize,
        count: usize,
    ) -> Result<Vec<FeedSummary>, QueryError> {
        let mut select = self.connection.prepare(
            "SELECT id, title, source, status, error_code FROM feeds LIMIT :limit OFFSET :offset",
        )?;
        let rows = select.query_map(named_params![":limit": count, ":offset": offset,], |row| {
            Ok(FeedSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                source: row.get(2)?,
                status: FeedStatus::from_db(row.get(3)?, row.get(4)?),
            })
        })?;
        Ok(collect_results(rows)?)
    }
}

impl EpisodeSummariesQuery {
    fn build_where_clause(&self, query: &mut String) {
        if self.feed_id.is_some() {
            query.push_str(" WHERE feed_id = :feed_id")
        }
    }

    fn build_params<'a>(&'a self, params: &mut Vec<(&'static str, &'a dyn rusqlite::ToSql)>) {
        if let Some(ref feed_id) = self.feed_id {
            params.push((":feed_id", feed_id));
        }
    }
}

impl PagedQueryHandler<EpisodeSummariesQuery> for SqliteDataProvider {
    fn get_size(&self, request: EpisodeSummariesQuery) -> Result<usize, QueryError> {
        let mut sql = "SELECT COUNT(id) FROM episodes".to_string();
        request.build_where_clause(&mut sql);
        let mut statement = self.connection.prepare(&sql)?;

        let mut params = Vec::new();
        request.build_params(&mut params);
        Ok(statement.query_row(&*params, |row| row.get(0))?)
    }

    fn query(
        &self,
        request: EpisodeSummariesQuery,
        offset: usize,
        count: usize,
    ) -> Result<Vec<EpisodeSummary>, QueryError> {
        let mut sql = "SELECT id, feed_id, episode_number, title, is_new, is_finished, position, duration, error_code, publication_date, media_url FROM episodes".to_string();
        request.build_where_clause(&mut sql);
        sql.push_str(" LIMIT :limit OFFSET :offset");
        let mut statement = self.connection.prepare(&sql)?;

        let mut params = vec![
            (":limit", &count as &dyn rusqlite::ToSql),
            (":offset", &offset as &dyn rusqlite::ToSql),
        ];
        request.build_params(&mut params);
        let rows = statement.query_map(&*params, |row| {
            Ok(EpisodeSummary {
                id: row.get(0)?,
                feed_id: row.get(1)?,
                episode_number: row.get(2)?,
                title: row.get(3)?,
                is_new: row.get(4)?,
                status: EpisodeStatus::from_db(row.get(5)?, row.get(6)?),
                duration: row.get(7)?,
                playback_error: row.get::<_, Option<u32>>(8)?.map(PlaybackError::from_db),
                publication_date: row.get(9)?,
                media_url: row.get(10)?,
            })
        })?;
        Ok(collect_results(rows)?)
    }
}

fn collect_results<T, E>(items: impl IntoIterator<Item = Result<T, E>>) -> Result<Vec<T>, E> {
    let iter = items.into_iter();
    let mut result = Vec::with_capacity(iter.size_hint().0);
    for item in iter {
        result.push(item?);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{ConnectionError, SqliteDataProvider};
    use pretty_assertions::assert_eq;

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

    /*#[test]
    fn crud_operations() {
        let provider = SqliteDataProvider::connect(":memory:").unwrap();
        let id = provider
            .feeds()
            .create_pending("http://example.com/feed.xml")
            .unwrap();

        let count = provider.get_size(FeedSummariesQuery).unwrap();
        assert_eq!(count, 1);

        let summaries = provider.query(FeedSummariesQuery, 0, 100).unwrap();
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

        let summaries = provider.query(FeedSummariesQuery, 0, 100).unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, id);
        assert_eq!(summaries[0].title.as_deref(), Some("Title"));
        assert_eq!(&summaries[0].source, "http://example.com/feed.xml");
        assert_eq!(summaries[0].status, FeedStatus::Error(FeedError::NotFound));
    }*/

    /*#[test]
    fn crud_operations_on_episodes() {
        let provider = SqliteDataProvider::connect(":memory:").unwrap();
        let feed_id = provider
            .feeds()
            .create_pending("http://example.com/feed.xml")
            .unwrap();

        let episode_1_id = provider
            .episodes()
            .sync_metadata(
                feed_id,
                &EpisodeMetadata {
                    title: Some("title".to_string()),
                    description: Some("description".to_string()),
                    link: Some("link".to_string()),
                    guid: "guid-1".to_string(),
                    duration: None,
                    publication_date: None,
                    episode_number: Some(3),
                    media_url: "http://example.com/feed.xml".to_string(),
                },
            )
            .unwrap();

        let retrieved = provider
            .get_episode(episode_1_id)
            .unwrap()
            .unwrap();
        assert_eq!(retrieved.id, episode_1_id);
        assert_eq!(retrieved.feed_id, feed_id);
        assert_eq!(retrieved.episode_number, Some(3));
        assert_eq!(retrieved.title.as_deref(), Some("title"));
        assert_eq!(retrieved.description.as_deref(), Some("description"));
        assert_eq!(retrieved.link.as_deref(), Some("link"));
        assert_eq!(retrieved.is_new, true);
        assert_eq!(retrieved.status, EpisodeStatus::NotStarted);
        assert_eq!(retrieved.duration, None);
        assert_eq!(retrieved.playback_error, None);
        assert_eq!(retrieved.publication_date, None);
        assert_eq!(&retrieved.media_url, "http://example.com/feed.xml");

        let episode_1_id_updated = provider
            .episodes()
            .sync_metadata(
                feed_id,
                &EpisodeMetadata {
                    title: Some("title-upd".to_string()),
                    description: Some("description-upd".to_string()),
                    link: Some("link-upd".to_string()),
                    guid: "guid-1".to_string(),
                    duration: Some(EpisodeDuration::from_seconds(300)),
                    publication_date: None,
                    episode_number: Some(8),
                    media_url: "http://example.com/feed2.xml".to_string(),
                },
            )
            .unwrap();
        assert_eq!(episode_1_id, episode_1_id_updated);

        let retrieved = provider
            .episodes()
            .get_episode(episode_1_id)
            .unwrap()
            .unwrap();
        assert_eq!(retrieved.id, episode_1_id);
        assert_eq!(retrieved.feed_id, feed_id);
        assert_eq!(retrieved.episode_number, Some(8));
        assert_eq!(retrieved.title.as_deref(), Some("title-upd"));
        assert_eq!(retrieved.description.as_deref(), Some("description-upd"));
        assert_eq!(retrieved.link.as_deref(), Some("link-upd"));
        assert_eq!(retrieved.is_new, true);
        assert_eq!(retrieved.status, EpisodeStatus::NotStarted);
        assert_eq!(retrieved.duration, Some(EpisodeDuration::from_seconds(300)));
        assert_eq!(retrieved.playback_error, None);
        assert_eq!(retrieved.publication_date, None);
        assert_eq!(&retrieved.media_url, "http://example.com/feed2.xml");

        let episode_2_id = provider
            .episodes()
            .sync_metadata(
                feed_id,
                &EpisodeMetadata {
                    title: Some("second-title".to_string()),
                    description: Some("second-description".to_string()),
                    link: None,
                    guid: "guid-2".to_string(),
                    duration: None,
                    publication_date: None,
                    episode_number: None,
                    media_url: "http://example.com/feed3.xml".to_string(),
                },
            )
            .unwrap();

        let mut episodes = provider.episodes().query_episodes().unwrap();
        episodes.sort_by_key(|episode| episode.id.0);
        assert_eq!(
            episodes[0],
            EpisodeSummary {
                id: episode_1_id,
                feed_id,
                episode_number: Some(8),
                title: Some("title-upd".to_string()),
                is_new: true,
                status: EpisodeStatus::NotStarted,
                duration: Some(EpisodeDuration::from_seconds(300)),
                playback_error: None,
                publication_date: None,
                media_url: "http://example.com/feed2.xml".to_string(),
            }
        );
        assert_eq!(
            episodes[1],
            EpisodeSummary {
                id: episode_2_id,
                feed_id,
                episode_number: None,
                title: Some("second-title".to_string()),
                is_new: true,
                status: EpisodeStatus::NotStarted,
                duration: None,
                playback_error: None,
                publication_date: None,
                media_url: "http://example.com/feed3.xml".to_string(),
            }
        );
    }*/
}
