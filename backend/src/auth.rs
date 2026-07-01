use crate::state::AppState;
use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header, request::Parts, HeaderValue, StatusCode},
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Name of the cookie carrying the JWT. `HttpOnly` so it's never reachable
/// from JS (unlike `localStorage`), which is the whole point - it keeps the
/// token safe even if an XSS bug lets an attacker run script on the page.
pub const AUTH_COOKIE: &str = "auth_token";

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub exp: usize,
}

/// Builds the `Set-Cookie` header value that logs a session in, valid for
/// `max_age_secs`.
pub fn build_auth_cookie(token: &str, max_age_secs: i64, secure: bool) -> HeaderValue {
    let secure_attr = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{AUTH_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age_secs}{secure_attr}"
    ))
    .expect("cookie value is ASCII")
}

/// Builds the `Set-Cookie` header value that clears the session cookie.
pub fn build_logout_cookie(secure: bool) -> HeaderValue {
    let secure_attr = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{AUTH_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{secure_attr}"
    ))
    .expect("cookie value is ASCII")
}

fn token_from_cookies(parts: &Parts) -> Option<&str> {
    let cookie_header = parts.headers.get(header::COOKIE)?.to_str().ok()?;
    cookie_header.split(';').find_map(|kv| {
        let (name, value) = kv.trim().split_once('=')?;
        (name == AUTH_COOKIE).then_some(value)
    })
}

#[allow(dead_code)]
pub struct AuthUser {
    pub id: String,
    pub email: String,
}

impl<S> FromRequestParts<S> for AuthUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);

        let token = token_from_cookies(parts)
            .ok_or((StatusCode::UNAUTHORIZED, "Missing authorization cookie"))?;

        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(app_state.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid token"))?;

        Ok(AuthUser {
            id: token_data.claims.sub,
            email: token_data.claims.email,
        })
    }
}

pub struct UserRow {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for UserRow {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(UserRow {
            id: row.try_get("id")?,
            email: row.try_get("email")?,
            password_hash: row.try_get("password_hash")?,
            created_at: row.try_get("created_at")?,
        })
    }
}
