mod birthdays;
mod devices;
mod login;
mod proxy;
mod screens;
mod transport;
mod users;
mod weather;

use crate::state::AppState;
use axum::{
    routing::{any, get, post, put},
    Router,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/devices", get(devices::list_devices))
        .route("/api/devices/{id}/save", post(devices::save_device))
        .route("/api/devices/{id}/push-screen", post(devices::push_screen))
        .route("/api/auth/login", post(login::login))
        .route(
            "/api/users",
            get(users::list_users).post(users::create_user),
        )
        .route(
            "/api/screens",
            get(screens::list_screens).post(screens::create_screen),
        )
        .route("/api/screens/default", get(screens::get_default_screen))
        .route(
            "/api/screens/{id}/set-default",
            put(screens::set_default_screen),
        )
        .route(
            "/api/screens/{id}",
            get(screens::get_screen)
                .put(screens::update_screen)
                .delete(screens::delete_screen),
        )
        .route("/api/weather", get(weather::get_weather))
        .route("/api/birthdays/template", get(birthdays::get_template))
        .route("/api/birthdays/import", post(birthdays::import_xlsx))
        .route("/api/transport/departures", get(transport::get_departures))
        .route("/api/transport/stops", get(transport::search_stops))
        .route("/api/iframe-proxy/{*path}", any(proxy::proxy_all))
        .route("/ws", get(devices::ws_handler))
}
