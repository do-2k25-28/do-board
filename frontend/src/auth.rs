fn get_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

pub fn get_token() -> Option<String> {
    get_storage()?.get_item("auth_token").ok()?
}

pub fn set_token(token: &str) {
    if let Some(s) = get_storage() {
        let _ = s.set_item("auth_token", token);
    }
}

pub fn clear_token() {
    if let Some(s) = get_storage() {
        let _ = s.remove_item("auth_token");
    }
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

pub fn logout() {
    clear_token();
    clear_email();
}

/// Reads the `exp` claim (seconds since epoch) out of a JWT without
/// verifying its signature - only used client-side to know when to
/// proactively sign the user out, the backend remains the source of truth.
fn token_exp_secs(token: &str) -> Option<i64> {
    let payload_b64 = token.split('.').nth(1)?;
    let mut padded = payload_b64.replace('-', "+").replace('_', "/");
    while padded.len() % 4 != 0 {
        padded.push('=');
    }
    let json = web_sys::window()?.atob(&padded).ok()?;
    let value: serde_json::Value = serde_json::from_str(&json).ok()?;
    value.get("exp")?.as_i64()
}

/// True if there's no token, or the stored token's `exp` claim is in the past.
pub fn is_token_expired() -> bool {
    let Some(token) = get_token() else {
        return true;
    };
    let Some(exp) = token_exp_secs(&token) else {
        return false;
    };
    let now_secs = js_sys::Date::now() / 1000.0;
    (exp as f64) <= now_secs
}
