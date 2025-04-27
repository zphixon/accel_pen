use crate::{
    config::{CONFIG, OAUTH_CLIENT_SECRET, UBI_PASSWORD},
    error::{ApiError, ApiErrorInner, Context},
    session::{NadeoTokenPair, TokenPair},
};
use serde::Deserialize;
use std::{sync::LazyLock, time::Duration};
use tokio::sync::RwLock;
use ts_rs::TS;
use url::Url;

pub const OAUTH_AUTHORIZE_URL: &str = "https://api.trackmania.com/oauth/authorize";
pub const OAUTH_GET_ACCESS_TOKEN_URL: &str = "https://api.trackmania.com/api/access_token";

pub static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent(&CONFIG.net.user_agent)
        //.timeout(Duration::from_secs(5))
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UbiToken {
    pub access_token: String,
    pub refresh_token: String,
}

pub struct UbiTokens {
    pub nadeo_services: UbiToken,
    pub nadeo_live_services: UbiToken,
}

const TICKET_URL: &str = "https://public-ubiservices.ubi.com/v3/profiles/sessions";
const TOKEN_URL: &str =
    "https://prod.trackmania.core.nadeo.online/v2/authentication/token/ubiservices";
const REFRESH_URL: &str =
    "https://prod.trackmania.core.nadeo.online/v2/authentication/token/refresh";

#[derive(Deserialize)]
struct UbiTicket {
    ticket: String,
}

async fn request_audience(ticket: &UbiTicket, audience: &str) -> Result<UbiToken, ApiError> {
    let body = format!(r#"{{"audience":"{}"}}"#, audience);
    tracing::trace!("Requesting audience: {}", body);
    Ok(CLIENT
        .clone()
        .post(TOKEN_URL)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("ubi_v1 t={}", ticket.ticket))
        .body(body)
        .send()
        .await
        .context(format!("Sending request for audience {}", audience))?
        .error_for_status()
        .context(format!(
            "Response from Nadeo for {} audience request",
            audience
        ))?
        .json()
        .await
        .context(format!("Parsing JSON for audience {}", audience))?)
}

async fn refresh_token(refresh_token: &str) -> Result<UbiToken, ApiError> {
    tracing::trace!("Requesting new tokens");
    let response = CLIENT
        .clone()
        .post(REFRESH_URL)
        // what is this bullshit
        .header("Authorization", format!("nadeo_v1 t={}", refresh_token))
        .send()
        .await
        .context("Sending request for refresh")?;

    if response.status().is_success() {
        response.json().await.context("Parsing JSON for refresh")
    } else {
        let error: serde_json::Value = response
            .json()
            .await
            .context("Parsing JSON from failed refresh")?;
        Err(ApiErrorInner::OauthFailed(format!("{}", error)).into())
    }
}

pub static UBI_TOKENS: LazyLock<RwLock<Option<UbiTokens>>> = LazyLock::new(|| RwLock::new(None));

pub async fn ubi_auth_task() -> Result<(), ApiError> {
    {
        let mut write = UBI_TOKENS.write().await;

        tracing::debug!("Getting ticket");
        let ticket: UbiTicket = CLIENT
            .clone()
            .post(TICKET_URL)
            .basic_auth(
                CONFIG.nadeo.ubi.username.as_str(),
                Some(UBI_PASSWORD.as_str()),
            )
            .header("Content-Type", "application/json")
            .header("Ubi-AppId", "86263886-327a-4328-ac69-527f0d20a237")
            .send()
            .await
            .context("Sending request for ticket")?
            .error_for_status()
            .context("Response from Ubi for ticket request")?
            .json()
            .await
            .context("Parsing JSON for ticket")?;
        tracing::debug!("Got ticket, getting audiences");

        let new = UbiTokens {
            nadeo_services: request_audience(&ticket, "NadeoServices")
                .await
                .context("Requesting NadeoServices audience")?,
            nadeo_live_services: request_audience(&ticket, "NadeoLiveServices")
                .await
                .context("Requesting NadeoLiveServices audience")?,
        };

        *write = Some(new);
        tracing::debug!("Got tokens, huzzah");
    }

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60 * 10)).await;
        tracing::debug!("Refreshing Ubisoft tokens");
        let new = UbiTokens {
            nadeo_services: refresh_token(
                &UBI_TOKENS
                    .read()
                    .await
                    .as_ref()
                    .expect("Should have ubi tokens?")
                    .nadeo_services
                    .refresh_token,
            )
            .await
            .context("Refreshing NadeoServices")?,
            nadeo_live_services: refresh_token(
                &UBI_TOKENS
                    .read()
                    .await
                    .as_ref()
                    .expect("Should still have ubi tokens????")
                    .nadeo_live_services
                    .refresh_token,
            )
            .await
            .context("Refreshing NadeoLiveServices")?,
        };
        *UBI_TOKENS
            .write()
            .await
            .as_mut()
            .expect("Definitely should still have ubi tokens????????") = new;
        tracing::debug!("Refreshed");
    }

    #[allow(unreachable_code)]
    Ok(())
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
