use std::sync::LazyLock;
use crate::config::CONFIG;



pub static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent(&CONFIG.net.user_agent)
        //.timeout(Duration::from_secs(5))
        .build()
        .expect("Could not build client for requests to Nadeo API")
});