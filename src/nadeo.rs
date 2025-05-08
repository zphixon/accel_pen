use crate::error::ApiError;
use base64::Engine;
use uuid::Uuid;

pub mod api;
pub mod auth;

pub fn login_to_account_id(login: &str) -> Result<String, ApiError> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(login)?;
    let hex_string = hex::encode(bytes);
    let uuid = Uuid::try_parse(&hex_string)?;
    Ok(uuid.hyphenated().to_string())
}

pub fn account_id_to_login(account_id: &str) -> Result<String, ApiError> {
    let _uuid = Uuid::parse_str(account_id)?;
    let bytes = hex::decode(account_id.replace("-", "")).expect("UUID not made of hex digits");
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}
