mod auth;
mod db;
mod fixtures;
mod gtfs;
mod pubsub;
mod routes;
mod screen_query;
mod state;

use axum::http::header::{ACCEPT, CONTENT_TYPE};
use sqlx::postgres::PgPoolOptions;
use state::{AppState, GtfsStore};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tower_http::cors::{AllowMethods, AllowOrigin, CorsLayer};

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let jwt_secret =
        std::env::var("JWT_SECRET").unwrap_or_else(|_| "change_me_in_production".to_string());
    let cookie_secure = std::env::var("COOKIE_SECURE")
        .map(|v| v == "true")
        .unwrap_or(false);

    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to database");

    db::run_migrations(&db).await;
    fixtures::run(&db).await;

    // GTFS static cache - loaded in background, refreshed every 24 h.
    // Requires GTFS_STATIC_URL env var pointing to TaM's GTFS ZIP.
    let gtfs_store: GtfsStore = Arc::new(tokio::sync::RwLock::new(None));
    if let Some(static_url) = gtfs::gtfs_static_url() {
        let store_clone = Arc::clone(&gtfs_store);
        tokio::spawn(async move {
            loop {
                if let Some(cache) = gtfs::load_static(&static_url).await {
                    *store_clone.write().await = Some(cache);
                } else {
                    eprintln!("[GTFS] Failed to load static data from {static_url}");
                }
                tokio::time::sleep(std::time::Duration::from_secs(24 * 3600)).await;
            }
        });
    } else {
        eprintln!("[GTFS] GTFS_STATIC_URL not set - transport slide will return 503");
    }

    let state = AppState {
        device_senders: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        db,
        jwt_secret,
        cookie_secure,
        gtfs: gtfs_store,
    };

    pubsub::spawn_device_push_listener(state.clone());

    // Credentialed requests (needed so the browser sends the HttpOnly auth
    // cookie) can't use a literal `*` for origin/methods per the Fetch spec -
    // mirror the request instead of hardcoding one origin, since the chart
    // supports an arbitrary `ingress.host`.
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::mirror_request())
        .allow_methods(AllowMethods::mirror_request())
        .allow_headers([CONTENT_TYPE, ACCEPT])
        .allow_credentials(true);

    let app = routes::router().layer(cors).with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Backend listening on http://0.0.0.0:3000");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
