use crate::auth::AuthUser;
use crate::state::AppState;
use axum::{
    extract::{
        ws::Message,
        {Path, State},
    },
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

    // Push updated screen to all connected devices currently showing it.
    // Collect device IDs first (drop the lock before acquiring device_senders).
    let devices_to_notify: Vec<String> = {
        let device_screens = state.device_screens.lock().await;
        device_screens
            .iter()
            .filter(|(_, sid)| *sid == &id)
            .map(|(did, _)| did.clone())
            .collect()
    };

    if !devices_to_notify.is_empty() {
        let msg_text =
            serde_json::to_string(&serde_json::json!({ "type": "set_screen", "screen": &screen }))
                .unwrap_or_default();

        let senders = state.device_senders.lock().await;
        for device_id in &devices_to_notify {
            if let Some(tx) = senders.get(device_id) {
                let _ = tx.send(Message::Text(msg_text.clone().into()));
            }
        }
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
