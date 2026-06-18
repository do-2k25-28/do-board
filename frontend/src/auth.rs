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
