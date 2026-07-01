use crate::auth::{self, Claims, UserRow};
use crate::state::AppState;
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use bcrypt::verify;
use jsonwebtoken::{encode, EncodingKey, Header};
use shared::{LoginRequest, LoginResponse, User};

const SESSION_MAX_AGE_SECS: i64 = 7 * 24 * 3600;

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<impl IntoResponse, (StatusCode, &'static str)> {
    let row = sqlx::query_as::<_, UserRow>(
        "SELECT id, email, password_hash, created_at FROM users WHERE email = $1",
    )
    .bind(&req.email)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
    .ok_or((StatusCode::UNAUTHORIZED, "Invalid credentials"))?;

    if !verify(&req.password, &row.password_hash)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Verification error"))?
    {
        return Err((StatusCode::UNAUTHORIZED, "Invalid credentials"));
    }

    let expires_at = chrono::Utc::now().timestamp() + SESSION_MAX_AGE_SECS;
    let claims = Claims {
        sub: row.id.to_string(),
        email: row.email.clone(),
        exp: expires_at as usize,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    )
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Token encoding error"))?;

    let cookie = auth::build_auth_cookie(&token, SESSION_MAX_AGE_SECS, state.cookie_secure);

    Ok((
        [(header::SET_COOKIE, cookie)],
        Json(LoginResponse {
            user: User {
                id: row.id.to_string(),
                email: row.email,
                created_at: row.created_at.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            },
            expires_at,
        }),
    ))
}

/// Clears the auth cookie. The client can't do this itself since the cookie
/// is `HttpOnly` (by design - that's what keeps it safe from XSS).
pub async fn logout(State(state): State<AppState>) -> impl IntoResponse {
    let cookie = auth::build_logout_cookie(state.cookie_secure);
    ([(header::SET_COOKIE, cookie)], StatusCode::NO_CONTENT)
}
