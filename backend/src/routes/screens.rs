use crate::auth::AuthUser;
use crate::pubsub;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use shared::{CreateScreenRequest, Screen, Slide, UpdateScreenRequest};
use sqlx::types::Json as SqlJson;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct ScreenRow {
    id: Uuid,
    name: String,
    slides: SqlJson<Vec<Slide>>,
    is_default: bool,
}

fn to_screen(row: ScreenRow) -> Screen {
    Screen {
        id: row.id.to_string(),
        name: row.name,
        slides: row.slides.0,
        is_default: row.is_default,
    }
}

fn parse_uuid(id: &str) -> Result<Uuid, (StatusCode, &'static str)> {
    Uuid::parse_str(id).map_err(|_| (StatusCode::BAD_REQUEST, "Invalid screen ID"))
}

pub async fn list_screens(
    _auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<Screen>>, (StatusCode, &'static str)> {
    let rows = sqlx::query_as::<_, ScreenRow>(
        "SELECT id, name, slides, is_default FROM screens ORDER BY created_at",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    Ok(Json(rows.into_iter().map(to_screen).collect()))
}

pub async fn get_default_screen(
    State(state): State<AppState>,
) -> Result<Json<Option<Screen>>, (StatusCode, &'static str)> {
    let row = sqlx::query_as::<_, ScreenRow>(
        "SELECT id, name, slides, is_default FROM screens WHERE is_default = TRUE LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    Ok(Json(row.map(to_screen)))
}

pub async fn set_default_screen(
    _auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    let uuid = parse_uuid(&id)?;

    sqlx::query("UPDATE screens SET is_default = FALSE")
        .execute(&state.db)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    let updated = sqlx::query("UPDATE screens SET is_default = TRUE WHERE id = $1")
        .bind(uuid)
        .execute(&state.db)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    if updated.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Screen not found"));
    }

    Ok(StatusCode::OK)
}

pub async fn create_screen(
    _auth: AuthUser,
    State(state): State<AppState>,
    Json(req): Json<CreateScreenRequest>,
) -> Result<Json<Screen>, (StatusCode, &'static str)> {
    let row = sqlx::query_as::<_, ScreenRow>(
        "INSERT INTO screens (name) VALUES ($1) RETURNING id, name, slides, is_default",
    )
    .bind(&req.name)
    .fetch_one(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    Ok(Json(to_screen(row)))
}

pub async fn get_screen(
    _auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Screen>, (StatusCode, &'static str)> {
    let uuid = parse_uuid(&id)?;
    let row = sqlx::query_as::<_, ScreenRow>(
        "SELECT id, name, slides, is_default FROM screens WHERE id = $1",
    )
    .bind(uuid)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
    .ok_or((StatusCode::NOT_FOUND, "Screen not found"))?;

    Ok(Json(to_screen(row)))
}

pub async fn update_screen(
    _auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateScreenRequest>,
) -> Result<Json<Screen>, (StatusCode, &'static str)> {
    let uuid = parse_uuid(&id)?;
    let row = sqlx::query_as::<_, ScreenRow>(
        "UPDATE screens SET name = $1, slides = $2
         WHERE id = $3 RETURNING id, name, slides, is_default",
    )
    .bind(&req.name)
    .bind(SqlJson(&req.slides))
    .bind(uuid)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
    .ok_or((StatusCode::NOT_FOUND, "Screen not found"))?;

    let screen = to_screen(row);

    // Push the updated screen to every device currently displaying it,
    // wherever (which replica) they happen to be connected.
    let devices_to_notify: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM devices WHERE current_screen_id = $1 AND online = TRUE")
            .bind(uuid)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();

    for device_id in devices_to_notify {
        let _ = pubsub::notify_device_push(&state.db, device_id, uuid).await;
    }

    Ok(Json(screen))
}

pub async fn delete_screen(
    _auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    let uuid = parse_uuid(&id)?;
    sqlx::query("DELETE FROM screens WHERE id = $1")
        .bind(uuid)
        .execute(&state.db)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    Ok(StatusCode::NO_CONTENT)
}
