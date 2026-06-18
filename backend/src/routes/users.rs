use crate::auth::{AuthUser, UserRow};
use crate::state::AppState;
use axum::{extract::State, http::StatusCode, Json};
use bcrypt::{hash, DEFAULT_COST};
use shared::{CreateUserRequest, User};

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
