use sqlx::PgPool;

pub async fn run_migrations(db: &PgPool) {
    sqlx::migrate!("./migrations")
        .run(db)
        .await
        .expect("Failed to run database migrations");
}
