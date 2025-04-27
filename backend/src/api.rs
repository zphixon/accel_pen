use crate::{
    auth::{
        nadeo::{NadeoTokens, NadeoTokensInner},
        ubi::UbiTokens,
    },
    config::CONFIG,
    error::{ApiError, ApiErrorInner, Context},
};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use url::Url;

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

    /// Takes NadeoTokensInner, because we would like NadeoTokens to have the account_id as well
    pub async fn get_self(token: &NadeoTokensInner) -> Result<User, ApiError> {
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

    const DISPLAY_NAMES_ENDPOINT: &str = "https://api.trackmania.com/api/display-names";

    pub async fn get_from_account_id(
        token: &NadeoTokensInner,
        account_id: &str,
    ) -> Result<User, ApiError> {
        let response: serde_json::Value = CLIENT
            .clone()
            .get(
                Url::parse_with_params(
                    Self::DISPLAY_NAMES_ENDPOINT,
                    &[("accountId[]", account_id)],
                )
                .context("Parse URL for display name request")?,
            )
            .bearer_auth(token.access_token())
            .send()
            .await
            .context("Sending request for user display name")?
            .error_for_status()
            .context("User display name returned non-OK status")?
            .json()
            .await
            .context("Parsing JSON response from Nadeo for user display name")?;

        let Some(object) = response.as_object() else {
            return Err(ApiErrorInner::UnexpectedResponse(
                "Response for display name was not an object",
            )
            .into());
        };
        let Some(display_name) = object.get(account_id) else {
            return Err(ApiErrorInner::UnexpectedResponse(
                "Response for display name did not have a display name for the account ID",
            )
            .into());
        };
        let Some(display_name) = display_name.as_str() else {
            return Err(ApiErrorInner::UnexpectedResponse(
                "Response for display name was not a string",
            )
            .into());
        };

        Ok(User {
            account_id: account_id.to_owned(),
            display_name: display_name.to_owned(),
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteMap {
    pub uid: String,
    pub name: String,
    pub author: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteMaps {
    pub list: Vec<FavoriteMap>,
}

impl FavoriteMaps {
    pub const ENDPOINT: &str = "https://api.trackmania.com/api/user/maps/favorite";

    pub async fn get(nadeo_token: &NadeoTokens) -> Result<FavoriteMaps, ApiError> {
        let response = CLIENT
            .clone()
            .get(Url::parse(Self::ENDPOINT).unwrap())
            .bearer_auth(nadeo_token.oauth_access_token())
            .send()
            .await
            .context("Sending request for favorite maps")?;

        if response.status().is_success() {
            Ok(response
                .json()
                .await
                .context("Reading JSON for favorite maps")?)
        } else {
            Err(ApiErrorInner::ApiReturnedError(
                response
                    .json::<serde_json::Value>()
                    .await
                    .context("Parsing error JSON")?,
            )
            .into())
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]

pub struct ClubTag {
    pub club_tag: String,
}

impl ClubTag {
    pub const ENDPOINT: &str = "https://prod.trackmania.core.nadeo.online/accounts/clubTags/";

    pub async fn get_self(nadeo_token: &NadeoTokens) -> Result<ClubTag, ApiError> {
        Self::get(nadeo_token.account_id()).await
    }

    pub async fn get(account_id: &str) -> Result<ClubTag, ApiError> {
        Ok(CLIENT
            .clone()
            .get(
                Url::parse_with_params(Self::ENDPOINT, &[("accountIdList", account_id)])
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

//#[derive(Deserialize)]
//#[serde(rename_all = "camelCase")]
//pub struct Top {
//    pub account_id: String,
//    pub zone_id: String,
//    pub zone_name: String,
//    pub position: u32,
//    pub score: u32,
//    pub timestamp: u64,
//}
//
//#[derive(Deserialize)]
//#[serde(rename_all = "camelCase")]
//pub struct Tops {
//    pub zone_id: String,
//    pub zone_name: String,
//    pub top: Vec<Top>,
//}
//
//#[derive(Deserialize)]
//#[serde(rename_all = "camelCase")]
//pub struct Leaderboard {
//    pub map_uid: String,
//    pub topses: Vec<Tops>,
//}
//
//pub enum GroupUid {
//    Global,
//}
//
//impl GroupUid {
//    fn as_str(&self) -> &'static str {
//        use GroupUid::*;
//        match self {
//            Global => "Personal_Best",
//        }
//    }
//}
//
//impl Leaderboard {
//    pub async fn get(group_uid: GroupUid, map_uid: &str) -> Result<Leaderboard, ApiError> {
//        let response = CLIENT.clone().get(
//            Url::parse_with_params(
//                &format!("https://live-services.trackmania.nadeo.live/api/token/leaderboard/group/{}/map/{}/top",
//                group_uid.as_str(),
//                map_uid
//            ), &[("onlyWorld", "true")]).unwrap())
//            .header("Authorization", format!("nadeo_v1 t={}", UbiTokens::nadeo_live_services().await))
//            .send().await.context("Sending leaderboard request")?;
//        if response.status().is_success() {
//            Ok(response
//                .json::<Leaderboard>()
//                .await
//                .context("Parsing JSON resposne for leaderboard request")?)
//        } else {
//            let json: serde_json::Value = response
//                .json()
//                .await
//                .context("Parsing error JSON for leaderboard request")?;
//            Err(ApiErrorInner::ApiReturnedError(json).into())
//        }
//    }
//}
