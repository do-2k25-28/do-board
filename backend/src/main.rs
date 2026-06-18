mod auth;
mod db;
mod fixtures;
mod routes;
mod state;

use sqlx::postgres::PgPoolOptions;
use state::AppState;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, RwLock},
};
use tower_http::cors::CorsLayer;

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

    let state = AppState {
        devices: Arc::new(RwLock::new(HashMap::new())),
        db,
        jwt_secret,
    };

    let app = routes::router()
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Backend listening on http://0.0.0.0:3000");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
