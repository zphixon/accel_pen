use crate::{
    config::{CONFIG, OAUTH_CLIENT_SECRET},
    error::{ApiError, ApiErrorInner, Context},
    session::{NadeoTokenPair, TokenPair},
};
use serde::Deserialize;
use std::sync::LazyLock;
use ts_rs::TS;
use url::Url;

pub const OAUTH_AUTHORIZE_URL: &str = "https://api.trackmania.com/oauth/authorize";
pub const OAUTH_GET_ACCESS_TOKEN_URL: &str = "https://api.trackmania.com/api/access_token";

pub static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent(&CONFIG.net.user_agent)
        .build()
        .expect("Could not build client for requests to Nadeo API")
});

pub async fn refresh(tokens: TokenPair) -> Result<TokenPair, ApiError> {
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
        let token_pair: NadeoTokenPair = response
            .json()
            .await
            .context("Parsing oauth tokens from Nadeo")?;
        Ok(TokenPair::from_nadeo(token_pair))
    } else {
        let json_error: serde_json::Value = response.json().await?;
        Err(ApiErrorInner::OauthFailed(format!("{}", json_error)).into())
    }
}

#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub account_id: String,
    pub display_name: String,
}

impl User {
    pub const ENDPOINT: &str = "https://api.trackmania.com/api/user";

    pub async fn get(token: &TokenPair) -> Result<User, ApiError> {
        Ok(CLIENT
            .clone()
            .get(User::ENDPOINT)
            .bearer_auth(&token.access_token())
            .send()
            .await
            .context("Sending request for user")?
            .error_for_status()
            .context("Returned non-OK status")?
            .json()
            .await
            .context("Parsing JSON response from Nadeo for user")?)
    }
}
