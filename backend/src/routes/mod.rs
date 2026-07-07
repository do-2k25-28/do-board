mod birthdays;
mod devices;
pub(crate) mod gamble;
pub(crate) mod interact;
mod login;
mod media;
mod proxy;
mod screens;
mod transport;
mod users;
mod weather;

use crate::state::AppState;
use axum::{
    extract::DefaultBodyLimit,
    routing::{any, get, post, put},
    Router,
};

/// Axum's implicit default body limit is 2MB, well under the 15MB image
/// cap enforced in `media::upload_image` - without raising it here, an
/// oversized request (e.g. an animated GIF) is rejected by axum before the
/// handler's own check ever runs.
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024; // 16 MB

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/devices", get(devices::list_devices))
        .route("/api/devices/{id}/save", post(devices::save_device))
        .route("/api/devices/{id}/push-screen", post(devices::push_screen))
        .route("/api/auth/login", post(login::login))
        .route("/api/auth/logout", post(login::logout))
        .route(
            "/api/users",
            get(users::list_users).post(users::create_user),
        )
        .route("/api/users/me/password", put(users::change_password))
        .route("/api/users/{id}/password", put(users::set_user_password))
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
        .route(
            "/api/screens/{id}/leaderboard",
            get(interact::get_screen_leaderboard),
        )
        .route("/api/weather", get(weather::get_weather))
        .route("/api/media", post(media::upload_image))
        .route("/api/media/{id}", get(media::get_media))
        .route("/api/birthdays/template", get(birthdays::get_template))
        .route("/api/birthdays/import", post(birthdays::import_xlsx))
        .route("/api/transport/departures", get(transport::get_departures))
        .route("/api/transport/stops", get(transport::search_stops))
        .route("/api/iframe-proxy/{*path}", any(proxy::proxy_all))
        .route("/api/interact/{session_id}", get(interact::get_interaction))
        .route(
            "/api/interact/{session_id}/respond",
            post(interact::respond),
        )
        .route("/api/gamble/{session_id}", get(gamble::get_table))
        .route("/api/gamble/{session_id}/join", post(gamble::join_table))
        .route("/api/gamble/{session_id}/action", post(gamble::play_action))
        .route("/ws", get(devices::ws_handler))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
}
