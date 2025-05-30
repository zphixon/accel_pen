use serde::Serialize;
use std::{net::SocketAddr, path::PathBuf, sync::LazyLock};
use tera::Context;
use url::Url;

from_env::config!(
    "ACCEL_PEN",
    dev_reload: bool,
    net {
        url: Url,
        bind: SocketAddr,
        user_agent: String,
    },
    db {
        url: Url,
        username: String,
        password_path: PathBuf,
    },
    nadeo {
        oauth {
            identifier: String,
            secret_path: PathBuf,
        },
        ubi {
            username: String,
            password_path: PathBuf,
        },
    },
);

impl Config {
    pub fn route_api_v1(&self, path: &str) -> String {
        format!("/api/v1{}", path)
    }

    pub fn oauth_start_route(&self) -> String {
        self.route_api_v1("/oauth/start")
    }

    pub fn oauth_finish_route(&self) -> String {
        self.route_api_v1("/oauth/finish")
    }

    pub fn oauth_logout_route(&self) -> String {
        self.route_api_v1("/oauth/logout")
    }

    pub fn oauth_redirect_url(&self) -> Url {
        self.net.url.join(&self.oauth_finish_route()).unwrap()
    }
}

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
        .set_username(&config.db.username)
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

pub static CONFIG_CONTEXT: LazyLock<Context> = LazyLock::new(|| {
    let mut context = Context::new();

    #[derive(Serialize)]
    struct ConfigContext {
        url: String,
        login_path: String,
        logout_path: String,
    }

    context.insert(
        "config",
        &ConfigContext {
            url: CONFIG.net.url.as_str().to_owned(),
            login_path: CONFIG.oauth_start_route(),
            logout_path: CONFIG.oauth_logout_route(),
        },
    );

    context
});

pub fn context_with_auth_session(auth: Option<&crate::nadeo::auth::NadeoAuthSession>) -> Context {
    let mut context = CONFIG_CONTEXT.clone();

    if let Some(auth) = auth {
        context.insert(
            "user",
            &Some(crate::routes::UserResponse {
                account_id: auth.account_id().to_owned(),
                display_name: auth.display_name().to_owned(),
                club_tag: auth.club_tag().map(crate::nadeo::FormattedString::parse),
                user_id: auth.user_id(),
                registered: Some(crate::routes::format_time(auth.registered())),
            }),
        );
    }

    context
}
