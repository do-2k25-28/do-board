mod dashboards;
mod devices;
mod login;
mod users;

use crate::state::AppState;
use axum::{
    routing::{get, post},
    Router,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/dashboards", get(dashboards::list_dashboards))
        .route("/api/devices", get(devices::list_devices))
        .route("/api/devices/{id}/save", post(devices::save_device))
        .route("/api/auth/login", post(login::login))
        .route(
            "/api/users",
            get(users::list_users).post(users::create_user),
        )
        .route("/ws", get(devices::ws_handler))
}
