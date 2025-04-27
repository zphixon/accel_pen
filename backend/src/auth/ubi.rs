use std::sync::LazyLock;

use serde::Deserialize;
use tokio::sync::RwLock;

use crate::{api::CLIENT, config::{CONFIG, UBI_PASSWORD}, error::{ApiError, ApiErrorInner, Context}};


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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UbiToken {
    access_token: String,
    refresh_token: String,
}

pub struct UbiTokens {
    nadeo_services: UbiToken,
    nadeo_live_services: UbiToken,
}

static UBI_TOKENS: LazyLock<RwLock<Option<UbiTokens>>> = LazyLock::new(|| RwLock::new(None));

impl UbiTokens {
    pub async fn nadeo_services() -> String {
        UbiTokens::ensure_tokens().await;
        UBI_TOKENS
            .read()
            .await
            .as_ref()
            .unwrap()
            .nadeo_services
            .access_token
            .clone()
    }

    pub async fn nadeo_live_services() -> String {
        UbiTokens::ensure_tokens().await;
        UBI_TOKENS
            .read()
            .await
            .as_ref()
            .unwrap()
            .nadeo_live_services
            .access_token
            .clone()
    }

    async fn ensure_tokens() {
        while UBI_TOKENS.read().await.is_none() {
            tokio::task::yield_now().await;
        }
    }

    /// Does not return - pass to tokio::spawn
    pub async fn auth_task() -> Result<(), ApiError> {
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
}