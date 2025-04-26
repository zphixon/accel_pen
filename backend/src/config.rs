use std::{net::SocketAddr, path::PathBuf, sync::LazyLock};
use url::Url;

from_env::config!(
    "ACCEL_PEN",
    net {
        domain: String,
        #[serde(default)]
        root: String,
        bind: SocketAddr,
        user_agent: String,
        #[serde(default = "default_cors_host")]
        cors_host: String,
        frontend_url: Url,
    },
    db {
        url: Url,
        password_path: PathBuf,
    },
    nadeo {
        identifier: String,
        secret_path: PathBuf,
        redirect_url: Url,
    }
);

fn default_cors_host() -> String {
    String::from("*")
}

impl Config {
    pub fn route_v1(&self, path: &str) -> String {
        format!("{}/v1/{}", self.net.root, path)
    }
}

pub static CLIENT_REDIRECT: LazyLock<String> = LazyLock::new(|| {
    format!(
        r#"<!DOCTYPE html>
<html>
<head><meta http-equiv="refresh" content="0; url='{}'"></head>
<body></body>
</html>
"#,
        CONFIG.net.frontend_url.as_str()
    )
});

pub static CLIENT_SECRET: LazyLock<String> = LazyLock::new(|| {
    let Ok(secret) = std::fs::read_to_string(&CONFIG.nadeo.secret_path) else {
        panic!("Couldn't read nadeo client secret file");
    };
    secret
});

pub static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    let arg = std::env::args().nth(1).expect("need config filename arg");
    let content = std::fs::read_to_string(arg).expect("could not read config file");

    let mut config = toml::from_str::<Config>(&content).expect("invalid TOML");
    config.hydrate_from_env();

    assert!(
        config.db.password_path.is_file(),
        "DB password path must be file"
    );

    config
        .db
        .url
        .set_username("root")
        .expect("Couldn't set username on DB URL");

    let Ok(password) = std::fs::read_to_string(&config.db.password_path) else {
        panic!("Couldn't read DB password file");
    };

    config
        .db
        .url
        .set_password(Some(password.trim()))
        .expect("Couldn't set password on DB URL");

    config
});
