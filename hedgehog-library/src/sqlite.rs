use crate::actor::UpdateQuery;
use crate::datasource::{
    DataProvider, DbResult, EpisodeWriter, EpisodesQuery, NewFeedMetadata, QueryError,
};
use crate::metadata::{EpisodeMetadata, FeedMetadata};
use crate::model::{
    Episode, EpisodeId, EpisodePlaybackData, EpisodeStatus, EpisodeSummary, EpisodeSummaryStatus,
    EpisodesListMetadata, Feed, FeedId, FeedOMPLEntry, FeedStatus, FeedSummary,
};
use rusqlite::{named_params, Connection};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::ops::Range;
use std::path::Path;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConnectionError {
    #[error("Database query failed")]
    SqliteError(#[from] rusqlite::Error),

    #[error("Database was updated in a newer version of hedgehog (db version: {version}, current: {version})")]
    VersionUnknown { version: u32, current: u32 },

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

        connection.execute("PRAGMA foreign_keys = ON", named_params! {})?;
        if version < 1 {
            connection.execute_batch(include_str!("schema/init.sql"))?;
        }

        connection.pragma_update(None, "user_version", Self::CURRENT_VERSION)?;
        Ok(SqliteDataProvider { connection })
    }
}

impl DataProvider for SqliteDataProvider {
    fn get_feed(&mut self, id: FeedId) -> DbResult<Option<crate::model::Feed>> {
        let mut statement = self.connection.prepare( "SELECT id, title, description, link, author, copyright, source, status, error_code FROM feeds WHERE id = ?1")?;
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

    fn get_feed_summaries(&mut self) -> DbResult<Vec<FeedSummary>> {
        let mut select = self
            .connection
            .prepare(
                "SELECT feeds.id, CASE WHEN feeds.title IS NOT NULL THEN feeds.title ELSE feeds.source END, 
                        feeds.title IS NOT NULL, feeds.status, feeds.error_code, COUNT(episodes.id)
                FROM feeds 
                LEFT JOIN episodes ON feeds.id = episodes.feed_id AND episodes.status = 0
                GROUP BY feeds.id
                ORDER BY feeds.title, feeds.source"
            )?;
        let rows = select.query_map([], |row| {
            Ok(FeedSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                has_title: row.get(2)?,
                status: FeedStatus::from_db(row.get(3)?, row.get(4)?),
                new_count: row.get(5)?,
            })
        })?;
        Ok(collect_results(rows)?)
    }

    fn get_feed_opml_entries(&mut self) -> DbResult<Vec<crate::model::FeedOMPLEntry>> {
        let mut select = self
            .connection
            .prepare("SELECT title, source, link FROM feeds")?;
        let rows = select.query_map([], |row| {
            Ok(FeedOMPLEntry {
                title: row.get(0)?,
                feed_source: row.get(1)?,
                link: row.get(2)?,
            })
        })?;
        Ok(collect_results(rows)?)
    }

    fn get_update_sources(&mut self, query: UpdateQuery) -> DbResult<Vec<(FeedId, String)>> {
        match query {
            UpdateQuery::Single(feed_id) => {
                let mut statement = self
                    .connection
                    .prepare("SELECT source FROM feeds WHERE id = :id LIMIT 1")?;
                let source =
                    statement.query_row(named_params! {":id": feed_id}, |row| row.get(0))?;
                Ok(vec![(feed_id, source)])
            }
            UpdateQuery::All => {
                let mut statement = self
                    .connection
                    .prepare("SELECT id, source FROM feeds WHERE enabled")?;
                let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
                Ok(collect_results(rows)?)
            }
            UpdateQuery::Pending => {
                let mut statement = self
                    .connection
                    .prepare("SELECT id, source FROM feeds WHERE enabled AND status = :status")?;
                let rows = statement.query_map(
                    named_params! {":status": FeedStatus::Pending.db_view().0},
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                Ok(collect_results(rows)?)
            }
        }
    }

    fn get_new_episodes_count(
        &mut self,
        feed_ids: HashSet<FeedId>,
    ) -> DbResult<HashMap<FeedId, usize>> {
        let mut sql = "
            SELECT feeds.id, COUNT(episodes.id)
            FROM feeds 
            LEFT JOIN episodes ON feeds.id = episodes.feed_id AND episodes.status = 0
            WHERE feeds.id IN ("
            .to_string();
        for (index, feed_id) in feed_ids.into_iter().enumerate() {
            if index == 0 {
                sql.write_fmt(format_args!("{}", feed_id.0)).unwrap();
            } else {
                sql.write_fmt(format_args!(", {}", feed_id.0)).unwrap();
            }
        }
        sql.push_str(") GROUP BY feeds.id");

        let mut select = self.connection.prepare(&sql)?;
        let rows = select.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;

        let mut results = HashMap::with_capacity(rows.size_hint().0);
        for row in rows {
            let (feed_id, count) = row?;
            results.insert(feed_id, count);
        }
        Ok(results)
    }

    fn get_episode(&mut self, episode_id: EpisodeId) -> DbResult<Option<Episode>> {
        let mut statement =
            self.connection.prepare("SELECT feed_id, episode_number, season_number, title, description, link, status, position, duration, publication_date, media_url FROM episodes WHERE id = :id")?;
        let result = statement.query_row(named_params! {":id": episode_id}, |row| {
            Ok(Episode {
                id: episode_id,
                feed_id: row.get(0)?,
                episode_number: row.get(1)?,
                season_number: row.get(2)?,
                title: row.get(3)?,
                description: row.get(4)?,
                link: row.get(5)?,
                status: EpisodeStatus::from_db(row.get(6)?, Duration::from_nanos(row.get(7)?)),
                duration: row.get::<_, Option<u64>>(8)?.map(Duration::from_nanos),
                publication_date: row.get(9)?,
                media_url: row.get(10)?,
            })
        });
        match result {
            Ok(episode) => Ok(Some(episode)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn get_episode_playback_data(
        &mut self,
        episode_id: EpisodeId,
    ) -> DbResult<Option<EpisodePlaybackData>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT episodes.media_url, episodes.position, episodes.duration, episodes.title, feeds.id, feeds.title
                FROM episodes JOIN feeds ON feeds.id = episodes.feed_id
                WHERE episodes.id = :id LIMIT 1")?;
        let result = statement.query_row(named_params! {":id": episode_id}, |row| {
            Ok(EpisodePlaybackData {
                id: episode_id,
                media_url: row.get(0)?,
                position: Duration::from_nanos(row.get(1)?),
                duration: row.get::<_, Option<u64>>(2)?.map(Duration::from_nanos),
                episode_title: row.get(3)?,
                feed_id: row.get(4)?,
                feed_title: row.get(5)?,
            })
        });
        match result {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn get_episodes_list_metadata(
        &mut self,
        query: EpisodesQuery,
    ) -> DbResult<EpisodesListMetadata> {
        let mut sql =
            "SELECT COUNT(ep.id), MAX(ep.season_number), MAX(ep.episode_number), MAX(ep.duration),
                    SUM(CASE WHEN ep.publication_date IS NOT NULL THEN 1 ELSE 0 END), feeds.reversed
            FROM episodes AS ep
            JOIN feeds ON ep.feed_id = feeds.id
            "
            .to_string();
        query.build_where_clause(&mut sql);
        let mut statement = self.connection.prepare(&sql)?;

        let where_params = EpisodeQueryParams::from_query(query);
        let params = where_params.as_sql_params();
        statement
            .query_row(&*params, |row| {
                Ok(EpisodesListMetadata {
                    items_count: row.get(0)?,
                    max_season_number: row.get(1)?,
                    max_episode_number: row.get(2)?,
                    max_duration: row.get::<_, Option<u64>>(3)?.map(Duration::from_nanos),
                    has_publication_date: row.get::<_, Option<u64>>(4)?.unwrap_or(0) > 0,
                    reversed_order: row.get::<_, Option<bool>>(5)?.unwrap_or(false),
                })
            })
            .map_err(QueryError::from)
    }

    fn get_episode_summaries(
        &mut self,
        request: EpisodesQuery,
        range: Range<usize>,
    ) -> DbResult<Vec<EpisodeSummary>> {
        let feed_title_required = request.include_feed_title;
        let mut sql = "SELECT ep.id, ep.feed_id, ep.episode_number, ep.season_number, ep.title, ep.status, ep.duration, ep.publication_date, ep.hidden".to_string();
        if feed_title_required {
            sql.push_str(", feeds.title");
        }
        sql.push_str(" FROM episodes AS ep");
        if feed_title_required {
            sql.push_str(" JOIN feeds ON feeds.id == ep.feed_id");
        }
        request.build_where_clause(&mut sql);
        sql.push_str(" ORDER BY ep.publication_date ");
        sql.push_str(match request.reversed_order {
            true => "ASC",
            false => "DESC",
        });
        sql.push_str(" LIMIT :limit OFFSET :offset");
        let mut statement = self.connection.prepare(&sql)?;

        let where_params = EpisodeQueryParams::from_query(request);
        let mut params = where_params.as_sql_params();
        let offset = range.start;
        let limit = range.end - range.start;
        params.push((":limit", &limit as &dyn rusqlite::ToSql));
        params.push((":offset", &offset as &dyn rusqlite::ToSql));
        let rows = statement.query_map(&*params, |row| {
            Ok(EpisodeSummary {
                id: row.get(0)?,
                feed_id: row.get(1)?,
                episode_number: row.get(2)?,
                season_number: row.get(3)?,
                title: row.get(4)?,
                status: EpisodeSummaryStatus::from_db(row.get(5)?),
                duration: row.get::<_, Option<u64>>(6)?.map(Duration::from_nanos),
                publication_date: row.get(7)?,
                feed_title: if feed_title_required {
                    row.get(9)?
                } else {
                    None
                },
                is_hidden: row.get(8)?,
            })
        })?;
        Ok(collect_results(rows)?)
    }

    fn count_episodes(&mut self, query: EpisodesQuery) -> DbResult<usize> {
        let mut sql = "SELECT COUNT(id) FROM episodes AS ep".to_string();
        query.build_where_clause(&mut sql);
        let mut statement = self.connection.prepare(&sql)?;

        let where_params = EpisodeQueryParams::from_query(query);
        let count = statement.query_row(&*where_params.as_sql_params(), |row| row.get(0))?;
        Ok(count)
    }

    fn create_feed_pending(&mut self, data: &NewFeedMetadata) -> DbResult<Option<FeedId>> {
        let mut exists_statement = self
            .connection
            .prepare("SELECT true FROM feeds WHERE source = :source")?;
        let exists = exists_statement
            .query(named_params! {":source": data.source})?
            .next()?
            .is_some();
        if exists {
            return Ok(None);
        }

        let mut statement = self
            .connection
            .prepare("INSERT INTO feeds (source, title, link) VALUES (:source, :title, :link)")?;
        statement
            .insert(
                named_params! {":source": data.source, ":title": data.title, ":link": data.link},
            )
            .map(|id| Some(FeedId(id)))
            .map_err(Into::into)
    }

    fn delete_feed(&mut self, id: FeedId) -> DbResult<()> {
        let mut statement = self
            .connection
            .prepare("DELETE FROM feeds WHERE id = :id")?;
        statement.execute(named_params! {":id": id})?;
        Ok(())
    }

    fn set_feed_status(&mut self, feed_id: FeedId, status: FeedStatus) -> DbResult<()> {
        let (status, error) = status.db_view();
        self.connection
            .prepare("UPDATE feeds SET status = :status, error_code = :error_code WHERE id = :id")?
            .execute(named_params! {":status": status, ":error_code": error, ":id": feed_id})?;
        Ok(())
    }

    fn set_feed_enabled(&mut self, feed_id: FeedId, enabled: bool) -> DbResult<()> {
        let mut statement = self
            .connection
            .prepare("UPDATE feeds SET enabled = :enabled WHERE id = :id")?;
        statement.execute(named_params! {":enabled": enabled, ":id": feed_id})?;
        Ok(())
    }

    fn reverse_feed_order(&mut self, feed_id: FeedId) -> DbResult<()> {
        let mut statement = self
            .connection
            .prepare("UPDATE feeds SET reversed = NOT reversed WHERE id = :feed_id")?;
        statement.execute(named_params! {":feed_id": feed_id})?;
        Ok(())
    }

    fn set_episode_status(
        &mut self,
        query: EpisodesQuery,
        status: EpisodeStatus,
    ) -> DbResult<HashSet<FeedId>> {
        let mut sql = "SELECT DISTINCT ep.feed_id FROM episodes AS ep ".to_string();
        query.build_where_clause(&mut sql);
        let where_params = EpisodeQueryParams::from_query(query.clone());
        let mut statement = self.connection.prepare(&sql)?;
        let feed_ids = statement.query_map(&*where_params.as_sql_params(), |row| row.get(0))?;
        let mut feed_ids_set = HashSet::new();
        for feed_id in feed_ids {
            feed_ids_set.insert(feed_id?);
        }

        let mut sql =
            "UPDATE episodes AS ep SET status = :new_status, position = :position ".to_string();
        query.build_where_clause(&mut sql);
        let mut statement = self.connection.prepare(&sql)?;

        let (status, position) = status.db_view();
        let position = position.as_nanos() as u64;
        let where_params = EpisodeQueryParams::from_query(query);
        let mut params = where_params.as_sql_params();
        params.push((":new_status", &status as &dyn rusqlite::ToSql));
        params.push((":position", &position as &dyn rusqlite::ToSql));
        statement.execute(&*params)?;

        Ok(feed_ids_set)
    }

    fn set_episode_hidden(&mut self, query: EpisodesQuery, hidden: bool) -> DbResult<()> {
        let mut sql = "UPDATE episodes AS ep SET hidden = :hidden".to_string();
        query.build_where_clause(&mut sql);
        let mut statement = self.connection.prepare(&sql)?;

        let where_params = EpisodeQueryParams::from_query(query);
        let mut params = where_params.as_sql_params();
        params.push((":hidden", &hidden));

        statement.execute(&*params)?;
        Ok(())
    }

    fn writer<'a>(&'a mut self, feed_id: FeedId) -> DbResult<Box<dyn EpisodeWriter + 'a>> {
        let transaction = self.connection.transaction()?;
        Ok(Box::new(SqliteEpisodeWriter {
            feed_id,
            transaction,
        }))
    }
}

impl EpisodesQuery {
    fn build_where_clause(&self, query: &mut String) {
        let mut clauses = Vec::new();
        if self.episode_id.is_some() {
            clauses.push("ep.id = :id");
        }
        if self.feed_id.is_some() {
            clauses.push("ep.feed_id = :feed_id");
        }
        if self.status.is_some() {
            clauses.push("ep.status = :status");
        }
        if !self.with_hidden {
            clauses.push("NOT ep.hidden");
        }
        if !clauses.is_empty() {
            query.push_str(" WHERE ");
            for (index, clause) in clauses.into_iter().enumerate() {
                if index > 0 {
                    query.push_str(" AND ");
                }
                query.push_str(clause);
            }
        }
    }
}

#[derive(Default)]
struct EpisodeQueryParams {
    id: Option<EpisodeId>,
    feed_id: Option<FeedId>,
    status: Option<usize>,
}

impl EpisodeQueryParams {
    fn from_query(query: EpisodesQuery) -> Self {
        EpisodeQueryParams {
            id: query.episode_id,
            feed_id: query.feed_id,
            status: query.status.map(|status| status.db_view()),
        }
    }

    fn as_sql_params<'a>(&'a self) -> Vec<(&'static str, &'a dyn rusqlite::ToSql)> {
        let mut params: Vec<(&'static str, &'a dyn rusqlite::ToSql)> = Vec::new();
        if let Some(id) = self.id.as_ref() {
            params.push((":id", id));
        }
        if let Some(feed_id) = self.feed_id.as_ref() {
            params.push((":feed_id", feed_id));
        }
        if let Some(status) = self.status.as_ref() {
            params.push((":status", status));
        }
        params
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

pub struct SqliteEpisodeWriter<'a> {
    feed_id: FeedId,
    transaction: rusqlite::Transaction<'a>,
}

impl<'a> EpisodeWriter for SqliteEpisodeWriter<'a> {
    fn set_feed_metadata(&mut self, metadata: &FeedMetadata) -> DbResult<()> {
        let mut statement = self.transaction.prepare(
            "UPDATE feeds
            SET title = :title, description = :description, link = :link, author = :author,
                copyright = :copyright, status = :status, error_code = :error_code
            WHERE id = :id",
        )?;
        let (status, error_code) = FeedStatus::Loaded.db_view();
        statement.execute(named_params! {
            ":title": metadata.title,
            ":description": metadata.description,
            ":link": metadata.link,
            ":author": metadata.author,
            ":copyright": metadata.copyright,
            ":status": status,
            ":error_code": error_code,
            ":id": self.feed_id
        })?;
        Ok(())
    }

    fn set_episode_metadata(&mut self, metadata: &EpisodeMetadata) -> DbResult<EpisodeId> {
        let mut statement = self.transaction.prepare(
            "INSERT INTO episodes (feed_id, guid, title, description, link, duration, publication_date, episode_number, season_number, media_url)
            VALUES (:feed_id, :guid, :title, :description, :link, :duration, :publication_date, :episode_number, :season_number, :media_url)
            ON CONFLICT (feed_id, guid) DO UPDATE SET
            title = :title, description = :description, link = :link, duration = :duration, publication_date = :publication_date, 
            episode_number = :episode_number, season_number = :season_number, media_url = :media_url
            WHERE feed_id = :feed_id AND guid = :guid"
        )?;
        statement.execute(named_params! {
            ":feed_id": self.feed_id,
            ":guid": metadata.guid,
            ":title": metadata.title,
            ":description": metadata.description,
            ":link": metadata.link,
            ":duration": metadata.duration.map(|duration|duration.as_nanos() as u64),
            ":publication_date": metadata.publication_date,
            ":episode_number": metadata.episode_number,
            ":season_number": metadata.season_number,
            ":media_url": metadata.media_url
        })?;

        let mut id_statement = self.transaction.prepare(
            "SELECT ep.id FROM episodes AS ep WHERE feed_id = :feed_id AND guid = :guid",
        )?;
        id_statement
            .query_row(
                named_params! {
                    ":feed_id": self.feed_id,
                    ":guid": metadata.guid,
                },
                |row| row.get(0),
            )
            .map_err(QueryError::from)
    }

    fn close(self: Box<Self>) -> DbResult<()> {
        self.transaction.commit().map_err(QueryError::from)
    }

    fn delete_episode(&mut self, guid: &str) -> DbResult<()> {
        let mut statement = self
            .transaction
            .prepare("DELETE FROM episodes WHERE feed_id = :feed_id AND guid = :guid")?;
        statement.execute(named_params! { ":feed_id": self.feed_id, ":guid": guid })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ConnectionError, SqliteDataProvider};
    use crate::datasource::{DataProvider, NewFeedMetadata};
    use crate::metadata::{EpisodeMetadata, FeedMetadata};
    use crate::model::{EpisodeStatus, EpisodeSummary, EpisodeSummaryStatus, FeedStatus};
    use crate::EpisodesQuery;
    use pretty_assertions::assert_eq;
    use std::time::Duration;

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
    fn feed_update() {
        let mut provider = SqliteDataProvider::connect(":memory:").unwrap();
        let id = provider
            .create_feed_pending(&NewFeedMetadata::new(
                "http://example.com/feed.xml".to_string(),
            ))
            .unwrap()
            .unwrap();

        let feed_summaries = provider.get_feed_summaries().unwrap();
        assert_eq!(feed_summaries.len(), 1);
        assert_eq!(feed_summaries[0].id, id);
        assert_eq!(feed_summaries[0].title, "http://example.com/feed.xml");
        assert_eq!(feed_summaries[0].has_title, false);
        assert_eq!(feed_summaries[0].status, FeedStatus::Pending);

        let mut writer = provider.writer(id).unwrap();
        writer
            .set_feed_metadata(&FeedMetadata {
                title: "Title",
                description: "Description",
                link: "http://example.com",
                author: Some("Author"),
                copyright: Some("Copyright"),
            })
            .unwrap();
        writer.close().unwrap();

        let feed = provider.get_feed(id).unwrap().unwrap();
        assert_eq!(feed.title.as_deref(), Some("Title"));
        assert_eq!(feed.description.as_deref(), Some("Description"));
        assert_eq!(feed.link.as_deref(), Some("http://example.com"));
        assert_eq!(feed.author.as_deref(), Some("Author"));
        assert_eq!(feed.copyright.as_deref(), Some("Copyright"));
        assert_eq!(&feed.source, "http://example.com/feed.xml");
        assert_eq!(feed.status, FeedStatus::Loaded);
    }

    #[test]
    fn does_not_create_duplicate() {
        let mut provider = SqliteDataProvider::connect(":memory:").unwrap();
        let id1 = provider
            .create_feed_pending(&NewFeedMetadata::new(
                "http://example.com/feed.xml".to_string(),
            ))
            .unwrap();
        let id2 = provider
            .create_feed_pending(&NewFeedMetadata::new(
                "http://example.com/feed.xml".to_string(),
            ))
            .unwrap();

        assert!(id1.is_some());
        assert!(id2.is_none());
    }

    #[test]
    fn episode_update() {
        let mut provider = SqliteDataProvider::connect(":memory:").unwrap();
        let feed_id = provider
            .create_feed_pending(&NewFeedMetadata::new(
                "http://example.com/feed.xml".to_string(),
            ))
            .unwrap()
            .unwrap();

        let mut writer = provider.writer(feed_id).unwrap();
        let episode_id = writer
            .set_episode_metadata(&EpisodeMetadata {
                title: Some("title"),
                description: Some("description"),
                link: Some("link"),
                guid: "guid-1",
                duration: None,
                publication_date: None,
                episode_number: Some(3),
                season_number: Some(4),
                media_url: "http://example.com/feed.xml",
                block: false,
            })
            .unwrap();
        writer.close().unwrap();

        let retrieved = provider.get_episode(episode_id).unwrap().unwrap();
        assert_eq!(retrieved.id, episode_id);
        assert_eq!(retrieved.feed_id, feed_id);
        assert_eq!(retrieved.episode_number, Some(3));
        assert_eq!(retrieved.season_number, Some(4));
        assert_eq!(retrieved.title.as_deref(), Some("title"));
        assert_eq!(retrieved.description.as_deref(), Some("description"));
        assert_eq!(retrieved.link.as_deref(), Some("link"));
        assert_eq!(retrieved.status, EpisodeStatus::New);
        assert_eq!(retrieved.duration, None);
        assert_eq!(retrieved.publication_date, None);
        assert_eq!(&retrieved.media_url, "http://example.com/feed.xml");

        let mut writer = provider.writer(feed_id).unwrap();
        let episode_id_1 = writer
            .set_episode_metadata(&EpisodeMetadata {
                title: Some("title-upd"),
                description: Some("description-upd"),
                link: Some("link-upd"),
                guid: "guid-1",
                duration: Some(Duration::from_secs(300)),
                publication_date: None,
                episode_number: Some(8),
                season_number: None,
                media_url: "http://example.com/feed2.xml",
                block: false,
            })
            .unwrap();
        assert_eq!(episode_id, episode_id_1);
        writer.close().unwrap();

        let retrieved = provider.get_episode(episode_id).unwrap().unwrap();
        assert_eq!(retrieved.id, episode_id);
        assert_eq!(retrieved.feed_id, feed_id);
        assert_eq!(retrieved.episode_number, Some(8));
        assert_eq!(retrieved.season_number, None);
        assert_eq!(retrieved.title.as_deref(), Some("title-upd"));
        assert_eq!(retrieved.description.as_deref(), Some("description-upd"));
        assert_eq!(retrieved.link.as_deref(), Some("link-upd"));
        assert_eq!(retrieved.status, EpisodeStatus::New);
        assert_eq!(retrieved.duration, Some(Duration::from_secs(300)));
        assert_eq!(retrieved.publication_date, None);
        assert_eq!(&retrieved.media_url, "http://example.com/feed2.xml");

        let mut writer = provider.writer(feed_id).unwrap();
        let episode_id_2 = writer
            .set_episode_metadata(&EpisodeMetadata {
                title: Some("second-title"),
                description: Some("second-description"),
                link: None,
                guid: "guid-2",
                duration: None,
                publication_date: None,
                episode_number: None,
                season_number: None,
                media_url: "http://example.com/feed3.xml",
                block: false,
            })
            .unwrap();
        writer.close().unwrap();

        let mut episodes = provider
            .get_episode_summaries(EpisodesQuery::default().feed_id(feed_id), 0..100)
            .unwrap();
        episodes.sort_by_key(|episode| episode.id.0);
        assert_eq!(
            episodes[0],
            EpisodeSummary {
                id: episode_id_1,
                feed_id,
                episode_number: Some(8),
                season_number: None,
                title: Some("title-upd".to_string()),
                feed_title: None,
                status: EpisodeSummaryStatus::New,
                duration: Some(Duration::from_secs(300)),
                publication_date: None,
                is_hidden: false,
            }
        );
        assert_eq!(
            episodes[1],
            EpisodeSummary {
                id: episode_id_2,
                feed_id,
                episode_number: None,
                season_number: None,
                title: Some("second-title".to_string()),
                feed_title: None,
                status: EpisodeSummaryStatus::New,
                duration: None,
                publication_date: None,
                is_hidden: false,
            }
        );
    }
}
