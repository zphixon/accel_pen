use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_sessions::Session;

#[derive(Debug, Serialize, Deserialize)]
struct NadeoOauth {
    token_type: String,
    expires_in: u64,
    access_token: String,
    refresh_token: String,
}

struct AuthenticatedSession {
    session: Session,
    tokens: Arc<NadeoOauth>,
}
