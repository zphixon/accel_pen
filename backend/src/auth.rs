use crate::{
    api::CLIENT, config::{CONFIG, UBI_PASSWORD}, error::{ApiError, ApiErrorInner, Context}
};
use nadeo::NadeoTokenPair;
use serde::Deserialize;
use std::sync::LazyLock;
use tokio::sync::RwLock;
use ts_rs::TS;

pub mod nadeo;
pub mod ubi;


#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub account_id: String,
    pub display_name: String,
}

impl User {
    pub const ENDPOINT: &str = "https://api.trackmania.com/api/user";

    pub async fn get(token: &NadeoTokenPair) -> Result<User, ApiError> {
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
