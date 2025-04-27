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
        cors_host: String,
        frontend_url: Url,
    },
    db {
        url: Url,
        password_path: PathBuf,
    },
    nadeo {
        oauth {
            identifier: String,
            secret_path: PathBuf,
            redirect_url: Url,
        },
        ubi {
            username: String,
            password_path: PathBuf,
        },
    },
);

impl Config {
    pub fn route_v1(&self, path: &str) -> String {
        format!("{}/v1/{}", self.net.root, path)
    }
}

pub static OAUTH_CLIENT_SECRET: LazyLock<String> = LazyLock::new(|| {
    let Ok(secret) = std::fs::read_to_string(&CONFIG.nadeo.oauth.secret_path) else {
        panic!("Couldn't read nadeo client secret file");
    };
    secret.trim().to_owned()
});

pub static UBI_PASSWORD: LazyLock<String> = LazyLock::new(|| {
    let Ok(password) = std::fs::read_to_string(&CONFIG.nadeo.ubi.password_path) else {
        panic!("Couldn't read Ubisoft password file");
    };
    password.trim().to_owned()
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
