use crate::datasource::{EpisodeSummariesQuery, FeedSummariesQuery, QueryError, QueryHandler};
use crate::model::{EpisodeStatus, EpisodeSummary, FeedStatus, FeedSummary, PlaybackError};
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

impl QueryHandler<FeedSummariesQuery> for SqliteDataProvider {
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

impl QueryHandler<EpisodeSummariesQuery> for SqliteDataProvider {
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
        sql.push_str(" LIMIT :limit OFFSER :offset");
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
