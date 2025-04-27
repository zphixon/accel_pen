use crate::{
    auth::{nadeo::{NadeoTokenPair, NadeoTokenPairInner}, ubi::UbiTokens},
    config::CONFIG,
    error::{ApiError, Context},
};
use serde::{Deserialize, Serialize};
use url::Url;
use std::sync::LazyLock;
use ts_rs::TS;

pub static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent(&CONFIG.net.user_agent)
        //.timeout(Duration::from_secs(5))
        .build()
        .expect("Could not build client for requests to Nadeo API")
});

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub account_id: String,
    pub display_name: String,
}

impl User {
    pub const ENDPOINT: &str = "https://api.trackmania.com/api/user";

    pub async fn get(token: &NadeoTokenPairInner) -> Result<User, ApiError> {
        Ok(CLIENT
            .clone()
            .get(User::ENDPOINT)
            .bearer_auth(token.access_token())
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]

pub struct ClubTag {
    pub club_tag: String,
}

impl ClubTag {
    pub const ENDPOINT: &str = "https://prod.trackmania.core.nadeo.online/accounts/clubTags/";

    pub async fn get(nadeo_token: &NadeoTokenPair) -> Result<ClubTag, ApiError> {
        Ok(CLIENT
            .clone()
            .get(
                Url::parse_with_params(
                    Self::ENDPOINT,
                    &[("accountIdList", nadeo_token.account_id())],
                )
                .context("Forming URL for request to get club tag")?,
            )
            .header(
                "Authorization",
                format!("nadeo_v1 t={}", UbiTokens::nadeo_services().await),
            )
            .send()
            .await
            .context("Sending request for club tag")?
            .json::<Vec<ClubTag>>()
            .await
            .context("Reading JSON for club tag response")?
            .pop()
            .unwrap())
    }
}
