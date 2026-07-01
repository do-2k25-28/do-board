use shared::Screen;
use sqlx::types::Json as SqlJson;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct ScreenRow {
    id: Uuid,
    name: String,
    slides: SqlJson<Vec<shared::Slide>>,
    is_default: bool,
}

pub async fn fetch_screen(db: &PgPool, id: Uuid) -> Result<Option<Screen>, sqlx::Error> {
    let row = sqlx::query_as::<_, ScreenRow>(
        "SELECT id, name, slides, is_default FROM screens WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;

    Ok(row.map(|r| Screen {
        id: r.id.to_string(),
        name: r.name,
        slides: r.slides.0,
        is_default: r.is_default,
    }))
}

pub async fn fetch_default_screen(db: &PgPool) -> Result<Option<Screen>, sqlx::Error> {
    let row = sqlx::query_as::<_, ScreenRow>(
        "SELECT id, name, slides, is_default FROM screens WHERE is_default = TRUE LIMIT 1",
    )
    .fetch_optional(db)
    .await?;

    Ok(row.map(|r| Screen {
        id: r.id.to_string(),
        name: r.name,
        slides: r.slides.0,
        is_default: r.is_default,
    }))
}

pub fn set_screen_message(screen: &Screen) -> String {
    serde_json::to_string(&serde_json::json!({ "type": "set_screen", "screen": screen }))
        .unwrap_or_default()
}
