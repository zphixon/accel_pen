use axum::{
    extract::DefaultBodyLimit,
    http::{header, HeaderValue, Method},
    routing::{get, post},
    Router,
};
use diesel_async::{
    pooled_connection::{deadpool::Pool, AsyncDieselConnectionManager},
    AsyncPgConnection,
};
use std::sync::Arc;
use tera::Tera;
use tokio::net::TcpListener;
use tower_http::{
    cors::{AllowMethods, CorsLayer},
    limit::RequestBodyLimitLayer,
    services::ServeDir,
    set_header::SetResponseHeaderLayer,
};
use tower_sessions::{
    cookie::{time::Duration, Key, SameSite},
    Expiry, MemoryStore, SessionManagerLayer,
};
use tracing_subscriber::EnvFilter;

mod config;
mod dev;
mod error;
mod models;
mod nadeo;
mod routes;
mod schema;
mod ubi;

use config::CONFIG;
use error::Context;
use ubi::UbiTokens;

#[derive(Clone)]
pub struct AppState {
    db: Pool<AsyncPgConnection>,
    tera: Arc<std::sync::RwLock<Tera>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        //.with_file(true)
        //.with_line_number(true)
        .init();

    tracing::info!("Bind on {}", CONFIG.net.bind);

    let tera = Tera::new("frontend/templates/**/*").context("Reading templates")?;
    tracing::debug!("Template names:");
    for template_name in tera.get_template_names() {
        tracing::debug!("  {}", template_name);
    }
    let tera = Arc::new(std::sync::RwLock::new(tera));

    if CONFIG.dev_reload {
        dev::reload_task(Arc::clone(&tera));
    }

    let ubi_auth_task = tokio::spawn(UbiTokens::auth_task());

    let server_task = tokio::spawn(async {
        let db = Pool::builder(AsyncDieselConnectionManager::<AsyncPgConnection>::new(
            CONFIG.db.url.as_str(),
        ))
        .build()?;

        let session_store = MemoryStore::default();
        let session_layer = SessionManagerLayer::new(session_store)
            .with_secure(false)
            .with_same_site(SameSite::Lax)
            .with_expiry(Expiry::OnInactivity(Duration::days(1)))
            .with_http_only(true)
            .with_private(Key::generate());

        let long_cache = SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("max-age=604800"),
        );

        let app = Router::new()
            .fallback(routes::web::my_fallback)
            .route("/", get(routes::web::index))
            .nest_service("/static", ServeDir::new("frontend/static"))
            .route("/map/upload", get(routes::web::map_upload))
            .route("/map/{map_id}", get(routes::web::map_page))
            .route("/map/{map_id}/manage", get(routes::web::map_manage_page))
            .route("/map/search", get(routes::web::map_search))
            .route("/user/{user_id}", get(routes::web::user_page))
            .route(
                &CONFIG.route_api_v1("/map/upload"),
                post(routes::api::map_upload),
            )
            .route(
                &CONFIG.route_api_v1("/map/{map_id}/thumbnail"),
                get(routes::api::map_thumbnail).layer(long_cache.clone()),
            )
            .route(
                &CONFIG.route_api_v1("/map/{map_id}/thumbnail/{size}"),
                get(routes::api::map_thumbnail_size).layer(long_cache),
            )
            .route(
                &CONFIG.route_api_v1("/map/{map_id}/manage"),
                post(routes::api::map_manage),
            )
            .route(
                &CONFIG.route_api_v1("/map/search"),
                post(routes::api::map_search),
            )
            .route(&CONFIG.oauth_start_route(), get(routes::api::oauth_start))
            .route(&CONFIG.oauth_finish_route(), get(routes::api::oauth_finish))
            .route(&CONFIG.oauth_logout_route(), get(routes::api::oauth_logout))
            .with_state(AppState { db, tera })
            .layer(session_layer)
            .layer(DefaultBodyLimit::disable())
            .layer(RequestBodyLimitLayer::new(20 * 1000 * 1000))
            .layer(
                CorsLayer::new()
                    .allow_origin(CONFIG.net.url.as_str().parse::<HeaderValue>()?)
                    .allow_methods(AllowMethods::list([Method::GET, Method::POST]))
                    .allow_credentials(true),
            );

        let listener = TcpListener::bind(CONFIG.net.bind).await?;
        axum::serve(listener, app).await?;

        Ok::<_, anyhow::Error>(())
    });

    tokio::select! {
        serve_result = server_task => {
            serve_result??;
        },
        ubi_auth_task_result = ubi_auth_task => {
            ubi_auth_task_result??;
        },
    }

    Ok(())
}

pub fn format_time(time: time::OffsetDateTime) -> String {
    time.format(&time::format_description::well_known::Iso8601::DATE_TIME_OFFSET)
        .context("Formatting map upload time")
        .expect("this is why i wanted to use regular Results for error handling")
}
