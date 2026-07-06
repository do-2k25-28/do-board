use crate::pubsub;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use shared::{
    DrawingStroke, InteractionInfo, InteractionKind, InteractionResponse, InteractionResults,
    InteractionSubmission, Slide, SlideConfig,
};
use sqlx::types::Json as SqlJson;
use uuid::Uuid;

const MAX_STAKE: u32 = 1000;
const MAX_STROKE_POINTS: usize = 2000;

fn kind_key(kind: &InteractionKind) -> &'static str {
    match kind {
        InteractionKind::Poll { .. } => "poll",
        InteractionKind::Bet { .. } => "bet",
        InteractionKind::Drawing { .. } => "drawing",
    }
}

/// Keeps `interaction_sessions` in sync with the interactive slides currently
/// saved on a screen. Called from `screens::update_screen` right after a
/// screen's slides are persisted.
pub async fn sync_sessions(
    db: &sqlx::PgPool,
    screen_id: Uuid,
    slides: &[Slide],
) -> Result<(), sqlx::Error> {
    for slide in slides {
        let SlideConfig::Interactive {
            session_id,
            interaction,
        } = &slide.config
        else {
            continue;
        };
        let Ok(session_uuid) = Uuid::parse_str(session_id) else {
            continue;
        };

        sqlx::query(
            "INSERT INTO interaction_sessions (id, screen_id, kind, config, updated_at)
             VALUES ($1, $2, $3, $4, NOW())
             ON CONFLICT (id) DO UPDATE SET
                 screen_id = EXCLUDED.screen_id,
                 kind = EXCLUDED.kind,
                 config = EXCLUDED.config,
                 updated_at = NOW()",
        )
        .bind(session_uuid)
        .bind(screen_id)
        .bind(kind_key(interaction))
        .bind(SqlJson(interaction))
        .execute(db)
        .await?;
    }

    Ok(())
}

#[derive(sqlx::FromRow)]
struct SessionRow {
    screen_id: Uuid,
    config: SqlJson<InteractionKind>,
}

async fn fetch_results(
    db: &sqlx::PgPool,
    session_id: Uuid,
    interaction: &InteractionKind,
) -> Result<InteractionResults, sqlx::Error> {
    match interaction {
        InteractionKind::Poll { options, .. } => {
            let rows: Vec<(i64, i64)> = sqlx::query_as(
                "SELECT (payload->>'option_index')::bigint AS idx, COUNT(*) AS n
                 FROM interaction_responses
                 WHERE session_id = $1
                 GROUP BY idx",
            )
            .bind(session_id)
            .fetch_all(db)
            .await?;

            let mut counts = vec![0u32; options.len()];
            for (idx, n) in rows {
                if let Some(slot) = usize::try_from(idx).ok().and_then(|i| counts.get_mut(i)) {
                    *slot = n as u32;
                }
            }
            Ok(InteractionResults::Poll { counts })
        }
        InteractionKind::Bet { options, .. } => {
            let rows: Vec<(i64, i64)> = sqlx::query_as(
                "SELECT (payload->>'option_index')::bigint AS idx, SUM((payload->>'stake')::bigint) AS total
                 FROM interaction_responses
                 WHERE session_id = $1
                 GROUP BY idx",
            )
            .bind(session_id)
            .fetch_all(db)
            .await?;

            let mut totals = vec![0u32; options.len()];
            for (idx, total) in rows {
                if let Some(slot) = usize::try_from(idx).ok().and_then(|i| totals.get_mut(i)) {
                    *slot = total as u32;
                }
            }
            Ok(InteractionResults::Bet { totals })
        }
        InteractionKind::Drawing { .. } => {
            let rows: Vec<(SqlJson<InteractionResponse>,)> = sqlx::query_as(
                "SELECT payload FROM interaction_responses
                 WHERE session_id = $1
                 ORDER BY created_at",
            )
            .bind(session_id)
            .fetch_all(db)
            .await?;

            let strokes: Vec<DrawingStroke> = rows
                .into_iter()
                .filter_map(|(payload,)| match payload.0 {
                    InteractionResponse::Drawing { stroke } => Some(stroke),
                    _ => None,
                })
                .collect();
            Ok(InteractionResults::Drawing { strokes })
        }
    }
}

/// Fresh `InteractionResults` for a session, or `None` if the session no
/// longer exists. Shared by the public `GET` handler and the pubsub listener
/// (`crate::pubsub::run_interaction_updates`), which re-fetches results
/// instead of having them embedded in the `NOTIFY` payload.
pub async fn compute_results(
    db: &sqlx::PgPool,
    session_id: Uuid,
) -> Result<Option<InteractionResults>, sqlx::Error> {
    let row: Option<SessionRow> =
        sqlx::query_as("SELECT screen_id, config FROM interaction_sessions WHERE id = $1")
            .bind(session_id)
            .fetch_optional(db)
            .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    Ok(Some(fetch_results(db, session_id, &row.config.0).await?))
}

pub async fn get_interaction(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<InteractionInfo>, StatusCode> {
    let session_uuid = Uuid::parse_str(&session_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    let row: Option<SessionRow> =
        sqlx::query_as("SELECT screen_id, config FROM interaction_sessions WHERE id = $1")
            .bind(session_uuid)
            .fetch_optional(&state.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let Some(row) = row else {
        return Err(StatusCode::NOT_FOUND);
    };

    let interaction = row.config.0;
    let results = fetch_results(&state.db, session_uuid, &interaction)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(InteractionInfo {
        interaction,
        results,
    }))
}

pub async fn respond(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(req): Json<InteractionSubmission>,
) -> Result<Json<InteractionResults>, (StatusCode, &'static str)> {
    let session_uuid = Uuid::parse_str(&session_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid session ID"))?;

    if req.participant_name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Missing participant name"));
    }

    let row: Option<SessionRow> =
        sqlx::query_as("SELECT screen_id, config FROM interaction_sessions WHERE id = $1")
            .bind(session_uuid)
            .fetch_optional(&state.db)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    let Some(row) = row else {
        return Err((StatusCode::NOT_FOUND, "Session not found"));
    };

    match (&row.config.0, &req.response) {
        (InteractionKind::Poll { options, .. }, InteractionResponse::Poll { option_index }) => {
            if *option_index >= options.len() {
                return Err((StatusCode::BAD_REQUEST, "Invalid option"));
            }
        }
        (
            InteractionKind::Bet { options, .. },
            InteractionResponse::Bet {
                option_index,
                stake,
            },
        ) => {
            if *option_index >= options.len() {
                return Err((StatusCode::BAD_REQUEST, "Invalid option"));
            }
            if *stake < 1 || *stake > MAX_STAKE {
                return Err((StatusCode::BAD_REQUEST, "Invalid stake"));
            }
        }
        (InteractionKind::Drawing { .. }, InteractionResponse::Drawing { stroke }) => {
            if stroke.points.is_empty() || stroke.points.len() > MAX_STROKE_POINTS {
                return Err((StatusCode::BAD_REQUEST, "Invalid stroke"));
            }
        }
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Response type doesn't match interaction kind",
            ))
        }
    }

    let payload = serde_json::to_value(&req.response).unwrap_or_default();
    sqlx::query(
        "INSERT INTO interaction_responses (session_id, participant_name, payload)
         VALUES ($1, $2, $3)",
    )
    .bind(session_uuid)
    .bind(&req.participant_name)
    .bind(payload)
    .execute(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    let results = fetch_results(&state.db, session_uuid, &row.config.0)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    let devices_to_notify: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM devices WHERE current_screen_id = $1 AND online = TRUE")
            .bind(row.screen_id)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();

    for device_id in devices_to_notify {
        let _ = pubsub::notify_interaction_update(&state.db, device_id, session_uuid).await;
    }

    Ok(Json(results))
}
