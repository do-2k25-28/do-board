mod auth;
mod db;
mod fixtures;
mod gtfs;
mod routes;
mod state;

use axum::http::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use sqlx::postgres::PgPoolOptions;
use state::{AppState, GtfsStore};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tower_http::cors::{Any, CorsLayer};

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let jwt_secret =
        std::env::var("JWT_SECRET").unwrap_or_else(|_| "change_me_in_production".to_string());

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
        devices: Arc::new(std::sync::RwLock::new(HashMap::new())),
        device_senders: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        device_screens: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        db,
        jwt_secret,
        gtfs: gtfs_store,
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        // Authorization must be listed explicitly - wildcards are not accepted
        // for credentialed headers by Firefox and other strict browsers.
        .allow_headers([AUTHORIZATION, CONTENT_TYPE, ACCEPT]);

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
