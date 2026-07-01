use crate::auth::AuthUser;
use crate::state::AppState;
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use uuid::Uuid;

const MAX_UPLOAD_BYTES: usize = 15 * 1024 * 1024; // 15 MB
const ALLOWED_CONTENT_TYPES: &[&str] = &[
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "image/svg+xml",
];

#[derive(Serialize)]
pub struct UploadResponse {
    pub url: String,
}

pub async fn upload_image(
    _auth: AuthUser,
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<UploadResponse>, (StatusCode, &'static str)> {
    if body.len() > MAX_UPLOAD_BYTES {
        return Err((StatusCode::PAYLOAD_TOO_LARGE, "Image too large (max 15MB)"));
    }
    if body.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Empty file"));
    }

    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !ALLOWED_CONTENT_TYPES.contains(&content_type) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Unsupported image type (allowed: PNG, JPEG, GIF, WEBP, SVG)",
        ));
    }

    let id: Uuid =
        sqlx::query_scalar("INSERT INTO media (content_type, data) VALUES ($1, $2) RETURNING id")
            .bind(content_type)
            .bind(body.as_ref())
            .fetch_one(&state.db)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    Ok(Json(UploadResponse {
        url: format!("/api/media/{id}"),
    }))
}

pub async fn get_media(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let row: Option<(String, Vec<u8>)> =
        sqlx::query_as("SELECT content_type, data FROM media WHERE id = $1")
            .bind(uuid)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None);

    match row {
        Some((content_type, data)) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, content_type),
                (
                    header::CACHE_CONTROL,
                    "public, max-age=31536000, immutable".to_string(),
                ),
            ],
            data,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
