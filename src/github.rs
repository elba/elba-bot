use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, FixedOffset};
use reqwest::{
    header::{self, HeaderMap},
    Client, StatusCode, Url,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use tokio::sync::RwLock;
use tokio::time::delay_for;

use crate::config::CONFIG;
use crate::error::{Error, Result};

pub const POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug)]
pub struct Github {
    client: Client,
    viewer_id: i64,
    etags: RwLock<HashMap<String, String>>,
}

impl Github {
    pub async fn new() -> Result<Self> {
        let client = Client::builder().build()?;
        let user: User = client
            .get(Url::parse(&url::authenticated_user())?)
            .headers(headers())
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(Self {
            client,
            viewer_id: user.id,
            etags: RwLock::new(HashMap::new()),
        })
    }

    pub fn viewer_id(&self) -> i64 {
        self.viewer_id
    }

    /// Query Github API V3 endpoint
    ///
    /// The ETAG header is used to prevent redundant query. Returns `None` when
    /// the content is unchanged.
    pub async fn query<T, Q>(&self, url: &str, query: &Q) -> Result<Option<GithubResponse<T>>>
    where
        T: DeserializeOwned,
        Q: Serialize,
    {
        let etag = self.etags.read().await.get(url).cloned();

        let mut headers = headers();
        if let Some(etag) = &etag {
            headers.insert(header::IF_NONE_MATCH, etag.parse().unwrap());
        }

        let resp = self
            .client
            .get(Url::parse(url)?)
            .query(query)
            .headers(headers)
            .send()
            .await?;

        match resp.status() {
            StatusCode::OK => (),
            StatusCode::NOT_MODIFIED => return Ok(None),
            _ => {
                let text = resp.text().await?;
                return Err(Error::Github(text).into());
            }
        }

        let etag = String::from_utf8(resp.headers().get(header::ETAG).unwrap().as_ref().to_vec())?;
        self.etags.write().await.insert(url.to_string(), etag);

        let date = DateTime::parse_from_rfc2822(&String::from_utf8(
            resp.headers().get(header::DATE).unwrap().as_ref().to_vec(),
        )?)?;
        let val = resp.json().await?;

        Ok(Some(GithubResponse { val, date }))
    }

    /// Query in endless loop until content changes are found
    pub async fn query_poll<T, Q>(
        &self,
        partial_url: String,
        query: &Q,
    ) -> Result<GithubResponse<T>>
    where
        T: DeserializeOwned,
        Q: Serialize,
    {
        loop {
            if let Some(resp) = self.query(&partial_url, query).await? {
                return Ok(resp);
            } else {
                delay_for(POLL_INTERVAL).await
            }
        }
    }

    pub async fn update_comment(&self, comment_id: i64, body: String) -> Result<()> {
        self.client
            .patch(Url::parse(&url::issue_comment(
                &CONFIG.index_repo_name,
                comment_id,
            ))?)
            .headers(headers())
            .json(&json!({ "body": body }))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

fn headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        format!("token {}", &CONFIG.access_token).parse().unwrap(),
    );
    headers.insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
    headers.insert(header::USER_AGENT, CONFIG.bot_name.parse().unwrap());
    headers
}

#[derive(Debug)]
pub struct GithubResponse<T> {
    pub val: T,
    pub date: DateTime<FixedOffset>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct User {
    pub id: i64,
    #[serde(rename = "login")]
    pub name: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Comment {
    pub id: i64,
    pub user: User,
    pub body: String,
    pub created_at: DateTime<FixedOffset>,
}

pub mod url {
    pub fn user_profile(user_name: &str) -> String {
        format!("https://github.com/{}", user_name)
    }

    pub fn authenticated_user() -> String {
        format!("https://api.github.com/user")
    }

    pub fn issue_comments(repo: &str, issue_number: &str) -> String {
        format!(
            "https://api.github.com/repos/{}/issues/{}/comments",
            repo, issue_number
        )
    }

    pub fn issue_comment(repo: &str, comment_id: i64) -> String {
        format!(
            "https://api.github.com/repos/{}/issues/comments/{}",
            repo, comment_id
        )
    }
}
