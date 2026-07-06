use sqlx::PgPool;

/// Applies every migration under `./migrations`, embedded at compile time.
/// (sqlx::migrate! re-reads this directory whenever this file is rebuilt.)
pub async fn run_migrations(db: &PgPool) {
    sqlx::migrate!("./migrations")
        .run(db)
        .await
        .expect("Failed to run database migrations");
}
