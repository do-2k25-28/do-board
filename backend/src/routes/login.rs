use crate::auth::{Claims, UserRow};
use crate::state::AppState;
use axum::{extract::State, http::StatusCode, Json};
use bcrypt::verify;
use jsonwebtoken::{encode, EncodingKey, Header};
use shared::{LoginRequest, LoginResponse, User};

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, &'static str)> {
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

    let exp = (chrono::Utc::now().timestamp() + 7 * 24 * 3600) as usize;
    let claims = Claims {
        sub: row.id.to_string(),
        email: row.email.clone(),
        exp,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    )
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Token encoding error"))?;

    Ok(Json(LoginResponse {
        token,
        user: User {
            id: row.id.to_string(),
            email: row.email,
            created_at: row.created_at.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        },
    }))
}
