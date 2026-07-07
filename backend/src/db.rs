use sqlx::PgPool;

/// Applies every migration under `./migrations`, embedded at compile time.
/// (sqlx::migrate! re-reads this directory whenever this file is rebuilt -
/// touch this file after adding a migration if it doesn't seem to apply.)
pub async fn run_migrations(db: &PgPool) {
    sqlx::migrate!("./migrations")
        .run(db)
        .await
        .expect("Failed to run database migrations");
}
