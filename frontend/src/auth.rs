use gloo_net::http::Request;
use web_sys::RequestCredentials;

const API_BASE: &str = match option_env!("API_BASE") {
    Some(v) => v,
    None => "",
};

const SESSION_EXPIRES_KEY: &str = "session_expires_at";

fn get_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

pub fn get_email() -> Option<String> {
    get_storage()?.get_item("auth_email").ok()?
}

pub fn set_email(email: &str) {
    if let Some(s) = get_storage() {
        let _ = s.set_item("auth_email", email);
    }
}

pub fn clear_email() {
    if let Some(s) = get_storage() {
        let _ = s.remove_item("auth_email");
    }
}

/// Records when the session cookie expires. This is a non-sensitive,
/// client-side *hint* only, used for UX (deciding whether to render the
/// dashboard immediately, and proactively redirecting to /login before the
/// user hits a 401). The actual credential is an HttpOnly cookie the client
/// never sees or controls - it, and every request the backend receives, is
/// the real security boundary.
pub fn set_session(expires_at: i64) {
    if let Some(s) = get_storage() {
        let _ = s.set_item(SESSION_EXPIRES_KEY, &expires_at.to_string());
    }
}

pub fn clear_session() {
    if let Some(s) = get_storage() {
        let _ = s.remove_item(SESSION_EXPIRES_KEY);
    }
}

/// Whether we have a locally-recorded session at all (doesn't guarantee the
/// cookie is still valid server-side - that's checked on every API call).
pub fn has_session_hint() -> bool {
    get_storage()
        .and_then(|s| s.get_item(SESSION_EXPIRES_KEY).ok().flatten())
        .is_some()
}

/// True if there's no recorded session, or its recorded expiry is in the past.
pub fn is_session_expired() -> bool {
    let Some(raw) = get_storage().and_then(|s| s.get_item(SESSION_EXPIRES_KEY).ok().flatten())
    else {
        return true;
    };
    let Ok(expires_at) = raw.parse::<i64>() else {
        return true;
    };
    let now_secs = js_sys::Date::now() / 1000.0;
    (expires_at as f64) <= now_secs
}

/// Clears the local session hint and asks the backend to drop the HttpOnly
/// auth cookie - the client can't clear it itself since `HttpOnly` blocks JS
/// access by design.
pub async fn logout() {
    clear_email();
    clear_session();
    let _ = Request::post(&format!("{API_BASE}/api/auth/logout"))
        .credentials(RequestCredentials::Include)
        .send()
        .await;
}
