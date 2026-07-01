use crate::auth::{AuthUser, UserRow};
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use bcrypt::{hash, verify, DEFAULT_COST};
use shared::{ChangePasswordRequest, CreateUserRequest, SetPasswordRequest, User};
use uuid::Uuid;

pub async fn list_users(
    _auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<User>>, (StatusCode, &'static str)> {
    let rows = sqlx::query_as::<_, UserRow>(
        "SELECT id, email, password_hash, created_at FROM users ORDER BY created_at",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    let users = rows
        .into_iter()
        .map(|row| User {
            id: row.id.to_string(),
            email: row.email,
            created_at: row.created_at.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        })
        .collect();

    Ok(Json(users))
}

pub async fn create_user(
    _auth: AuthUser,
    State(state): State<AppState>,
    Json(req): Json<CreateUserRequest>,
) -> Result<Json<User>, (StatusCode, &'static str)> {
    let password_hash = hash(&req.password, DEFAULT_COST)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Hash error"))?;

    let row = sqlx::query_as::<_, UserRow>(
        "INSERT INTO users (email, password_hash) VALUES ($1, $2)
         RETURNING id, email, password_hash, created_at",
    )
    .bind(&req.email)
    .bind(password_hash)
    .fetch_one(&state.db)
    .await
    .map_err(|_| (StatusCode::CONFLICT, "Email already exists"))?;

    Ok(Json(User {
        id: row.id.to_string(),
        email: row.email,
        created_at: row.created_at.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
    }))
}

pub async fn change_password(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    let user_id =
        Uuid::parse_str(&auth.id).map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid token"))?;

    let row = sqlx::query_as::<_, UserRow>(
        "SELECT id, email, password_hash, created_at FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
    .ok_or((StatusCode::UNAUTHORIZED, "Invalid token"))?;

    if !verify(&req.current_password, &row.password_hash)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Verification error"))?
    {
        return Err((StatusCode::UNAUTHORIZED, "Current password is incorrect"));
    }

    let new_hash = hash(&req.new_password, DEFAULT_COST)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Hash error"))?;

    sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
        .bind(new_hash)
        .bind(user_id)
        .execute(&state.db)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    Ok(StatusCode::NO_CONTENT)
}

/// Lets any signed-in (i.e. admin) user reset another user's password
/// without knowing their current one.
pub async fn set_user_password(
    _auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<SetPasswordRequest>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    let user_id = Uuid::parse_str(&id).map_err(|_| (StatusCode::BAD_REQUEST, "Invalid user ID"))?;

    let new_hash = hash(&req.new_password, DEFAULT_COST)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Hash error"))?;

    let result = sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
        .bind(new_hash)
        .bind(user_id)
        .execute(&state.db)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "User not found"));
    }

    Ok(StatusCode::NO_CONTENT)
}
