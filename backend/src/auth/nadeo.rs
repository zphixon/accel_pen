use serde::{Deserialize, Serialize};
use url::Url;
use crate::{config::{CONFIG, OAUTH_CLIENT_SECRET}, error::{ApiError, ApiErrorInner, Context}};
use super::CLIENT;


pub const OAUTH_AUTHORIZE_URL: &str = "https://api.trackmania.com/oauth/authorize";
pub const OAUTH_GET_ACCESS_TOKEN_URL: &str = "https://api.trackmania.com/api/access_token";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NadeoTokenPairInner {
    token_type: String,
    expires_in: u32,
    access_token: String,
    refresh_token: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct NadeoTokenPair {
    inner: NadeoTokenPairInner,
    issued: time::OffsetDateTime,
}

impl NadeoTokenPair {
    pub fn from_nadeo(nadeo_token_pair: NadeoTokenPairInner) -> Self {
        NadeoTokenPair {
            inner: nadeo_token_pair,
            issued: time::OffsetDateTime::now_utc(),
        }
    }

    pub fn refresh_token(&self) -> &str {
        &self.inner.refresh_token
    }

    pub fn access_token(&self) -> &str {
        &self.inner.access_token
    }

    pub fn expired(&self) -> bool {
        let margin = time::Duration::seconds(self.inner.expires_in.saturating_sub(30) as i64);
        let expiry = self.issued + margin;
        time::OffsetDateTime::now_utc() > expiry
    }
}

pub async fn refresh(tokens: NadeoTokenPair) -> Result<NadeoTokenPair, ApiError> {
    let params = form_urlencoded::Serializer::new(String::new())
        .append_pair("grant_type", "refresh_token")
        .append_pair("client_id", &CONFIG.nadeo.oauth.identifier)
        .append_pair("client_secret", &OAUTH_CLIENT_SECRET)
        .append_pair("refresh_token", tokens.refresh_token())
        .finish();

    let response = CLIENT
        .clone()
        .post(Url::parse(OAUTH_GET_ACCESS_TOKEN_URL).unwrap())
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(params)
        .send()
        .await
        .context("Sending request for refresh token")?;

    if response.status().is_success() {
        let token_pair: NadeoTokenPairInner = response
            .json()
            .await
            .context("Parsing oauth tokens from Nadeo")?;
        Ok(NadeoTokenPair::from_nadeo(token_pair))
    } else {
        let json_error: serde_json::Value = response.json().await?;
        Err(ApiErrorInner::OauthFailed(format!("{}", json_error)).into())
    }
}
