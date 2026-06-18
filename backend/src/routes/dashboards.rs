use axum::Json;
use shared::Dashboard;

pub async fn list_dashboards() -> Json<Vec<Dashboard>> {
    Json(vec![])
}
