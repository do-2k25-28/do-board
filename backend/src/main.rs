use axum::{routing::get, Json, Router};
use shared::Dashboard;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/api/dashboards", get(list_dashboards))
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Backend listening on http://0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}

async fn list_dashboards() -> Json<Vec<Dashboard>> {
    Json(vec![])
}
