use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
pub struct SearchResult {
    #[serde(rename = "collectionName")]
    pub title: String,

    #[serde(rename = "trackCount")]
    pub episodes_count: u64,

    #[serde(rename = "feedUrl")]
    #[serde(default)]
    pub feed_url: String,

    #[serde(rename = "artistName")]
    pub author: String,

    #[serde(rename = "primaryGenreName")]
    pub genre: String,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    results: Vec<SearchResult>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Networking(#[from] reqwest::Error),

    #[error("Invalid response: {0}")]
    InvalidResponse(reqwest::StatusCode),

    #[error("Invalid response body: {0}")]
    FormatError(#[from] serde_json::Error),
}

pub struct SearchClient {
    endpoint_url: Option<String>,
    client: reqwest::Client,
}

impl SearchClient {
    pub fn new() -> Self {
        SearchClient {
            endpoint_url: None,
            client: reqwest::Client::new(),
        }
    }

    pub fn with_endpoint_url(mut self, endpoint_url: String) -> Self {
        self.endpoint_url = Some(endpoint_url);
        self
    }

    pub async fn perform(&self, terms: &str) -> Result<Vec<SearchResult>, Error> {
        let response = self
            .client
            .get(
                self.endpoint_url
                    .as_deref()
                    .unwrap_or("https://itunes.apple.com/search"),
            )
            .query(&[("term", terms), ("entity", "podcast"), ("limit", "50")])
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(Error::InvalidResponse(response.status()));
        }

        let body = response.text().await?;
        let response: SearchResponse = serde_json::from_str(&body)?;
        let mut results = response.results;
        results.retain(|entry| !entry.feed_url.is_empty());
        Ok(results)
    }
}

impl Default for SearchClient {
    fn default() -> Self {
        SearchClient::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, SearchClient, SearchResult};

    #[actix::test]
    async fn search_success() {
        let server = httpmock::MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/search")
                .query_param("term", "query+terms")
                .query_param("entity", "podcast")
                .query_param("limit", "50");
            then.status(200)
                .body(include_str!("./test_data/itunes/success.json"));
        });

        let client = SearchClient::new().with_endpoint_url(format!("{}/search", server.base_url()));
        let result = client.perform("query terms").await.unwrap();
        assert_eq!(
            result,
            vec![
                SearchResult {
                    title: "HD - NASA's Jet Propulsion Laboratory".to_string(),
                    episodes_count: 100,
                    feed_url: "https://www.jpl.nasa.gov/multimedia/rss/podfeed-hd.xml".to_string(),
                    author: "High Definition Video".to_string(),
                    genre: "Science".to_string(),
                },
                SearchResult {
                    title: "NASA's Curious Universe".to_string(),
                    episodes_count: 29,
                    feed_url: "https://www.nasa.gov/rss/dyn/curious-universe.rss".to_string(),
                    author: "National Aeronautics and Space Administration (NASA)".to_string(),
                    genre: "Science".to_string(),
                },
                SearchResult {
                    title: "NASACast: This Week @NASA Audio".to_string(),
                    episodes_count: 10,
                    feed_url: "https://www.nasa.gov/rss/dyn/TWAN_podcast.rss".to_string(),
                    author: "National Aeronautics and Space Administration (NASA)".to_string(),
                    genre: "Science".to_string(),
                },
            ]
        );

        mock.assert();
    }

    #[actix::test]
    async fn search_success_null_feed_ull() {
        let server = httpmock::MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/search")
                .query_param("term", "query+terms")
                .query_param("entity", "podcast")
                .query_param("limit", "50");
            then.status(200)
                .body(include_str!("./test_data/itunes/success-with-no-feed.json"));
        });

        let client = SearchClient::new().with_endpoint_url(format!("{}/search", server.base_url()));
        let result = client.perform("query terms").await.unwrap();
        assert_eq!(
            result,
            vec![
                SearchResult {
                    title: "HD - NASA's Jet Propulsion Laboratory".to_string(),
                    episodes_count: 100,
                    feed_url: "https://www.jpl.nasa.gov/multimedia/rss/podfeed-hd.xml".to_string(),
                    author: "High Definition Video".to_string(),
                    genre: "Science".to_string(),
                },
                SearchResult {
                    title: "NASACast: This Week @NASA Audio".to_string(),
                    episodes_count: 10,
                    feed_url: "https://www.nasa.gov/rss/dyn/TWAN_podcast.rss".to_string(),
                    author: "National Aeronautics and Space Administration (NASA)".to_string(),
                    genre: "Science".to_string(),
                },
            ]
        );

        mock.assert();
    }

    #[actix::test]
    async fn search_empty() {
        let server = httpmock::MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/search")
                .query_param("term", "query+terms")
                .query_param("entity", "podcast")
                .query_param("limit", "50");
            then.status(200)
                .body(include_str!("./test_data/itunes/empty.json"));
        });

        let client = SearchClient::new().with_endpoint_url(format!("{}/search", server.base_url()));
        let result = client.perform("query terms").await.unwrap();
        assert!(result.is_empty());
        mock.assert();
    }

    #[actix::test]
    async fn service_unavailable() {
        let server = httpmock::MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/search")
                .query_param("term", "query+terms")
                .query_param("entity", "podcast")
                .query_param("limit", "50");
            then.status(500);
        });

        let client = SearchClient::new().with_endpoint_url(format!("{}/search", server.base_url()));
        let result = client.perform("query terms").await.unwrap_err();
        match result {
            Error::InvalidResponse(status_code) => {
                assert_eq!(status_code, reqwest::StatusCode::from_u16(500).unwrap());
            }
            error => panic!("Unexpected error: {:?}", error),
        }
    }
}
