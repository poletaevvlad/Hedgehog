use crate::model::{Episode, EpisodeId, EpisodeSummary, Feed, FeedId, FeedSummary};
use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum QueryError {
    #[error(transparent)]
    SqliteError(#[from] rusqlite::Error),
}

pub trait ListQuery: Send {
    type Item: 'static + Send;
}

pub trait QueryHandler<P: ListQuery> {
    fn get_size(&self, request: P) -> Result<usize, QueryError>;

    fn query(&self, request: P, offset: usize, count: usize) -> Result<Vec<P::Item>, QueryError>;
}

pub struct FeedSummariesQuery;

impl ListQuery for FeedSummariesQuery {
    type Item = FeedSummary;
}

#[derive(Debug, Clone)]
pub struct EpisodeSummariesQuery {
    pub feed_id: Option<FeedId>,
}

impl ListQuery for EpisodeSummariesQuery {
    type Item = EpisodeSummary;
}

pub trait DataProvider:
    std::marker::Unpin + QueryHandler<FeedSummariesQuery> + QueryHandler<EpisodeSummariesQuery>
{
    fn get_feed(&self, id: FeedId) -> Result<Option<Feed>, QueryError>;
    fn get_episode(&self, episode_id: EpisodeId) -> Result<Option<Episode>, QueryError>;
    fn create_feed_pending(&self, source: &str) -> Result<FeedId, QueryError>;
}

/*pub trait FeedsDao {
    fn update_metadata(
        &self,
        id: FeedId,
        metadata: &FeedMetadata,
    ) -> Result<bool, rusqlite::Error> {
        let mut statement = self.prepare(
            "UPDATE feeds
            SET title = :title, description = :description, link = :link, author = :author, copyright = :copyright, status = :status, error_code = :error_code
            WHERE id = :id"
        )?;
        let (status, error_code) = FeedStatus::Loaded.into_db();
        statement
            .execute(named_params! {
                ":title": metadata.title,
                ":description": metadata.description,
                ":link": metadata.link,
                ":author": metadata.author,
                ":copyright": metadata.copyright,
                ":status": status,
                ":error_code": error_code,
                ":id": id
            })
            .map(|updated| updated > 0)
    }

    fn update_status(&self, id: FeedId, status: FeedStatus) -> Result<bool, rusqlite::Error> {
        let mut statement = self.prepare(
            "UPDATE feeds SET status = :status, error_code = :error_code WHERE id = :id",
        )?;
        let (status, error_code) = status.into_db();
        statement
            .execute(named_params! {":status": status, ":error_code": error_code, ":id": id})
            .map(|updated| updated > 0)
    }

    fn delete(&self, id: FeedId) -> Result<bool, rusqlite::Error> {
        let mut statement = self.prepare("DELETE FROM feeds WHERE id = :id")?;
        statement
            .execute(named_params! {":id": id})
            .map(|updated| updated > 0)
    }
}

pub trait EpisodesDao {
    fn sync_metadata(
        &self,
        feed_id: FeedId,
        metadata: &EpisodeMetadata,
    ) -> Result<EpisodeId, rusqlite::Error> {
        self.prepare(
            "INSERT INTO episodes (feed_id, guid, title, description, link, duration, publication_date, episode_number, media_url)
            VALUES (:feed_id, :guid, :title, :description, :link, :duration, :publication_date, :episode_number, :media_url)
            ON CONFLICT (feed_id, guid) DO UPDATE SET
            title = :title, description = :description, link = :link, duration = :duration, publication_date = :publication_date, episode_number = :episode_number, media_url = :media_url
            WHERE feed_id = :feed_id AND guid = :guid
            RETURNING id",
        )?.query_row(named_params! {
            ":feed_id": feed_id,
            ":guid": metadata.guid,
            ":title": metadata.title,
            ":description": metadata.description,
            ":link": metadata.link,
            ":duration": metadata.duration,
            ":publication_date": metadata.publication_date,
            ":episode_number": metadata.episode_number,
            ":media_url": metadata.media_url
        }, |row| row.get(0))
    }

    fn get_episode(&self, episode_id: EpisodeId) -> Result<Option<Episode>, rusqlite::Error> {
        let mut statement =
            self.prepare("SELECT feed_id, episode_number, title, description, link, is_new, is_finished, position, duration, error_code, publication_date, media_url FROM episodes WHERE id = :id")?;
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
            Err(error) => Err(error),
        }
    }
}*/
