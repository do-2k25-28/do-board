use bcrypt::{hash, DEFAULT_COST};
use sqlx::PgPool;

pub async fn run(db: &PgPool) {
    let email = std::env::var("ADMIN_EMAIL").unwrap_or_else(|_| "admin@example.com".to_string());
    let password = std::env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "changeme".to_string());

    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)")
        .bind(&email)
        .fetch_one(db)
        .await
        .unwrap_or(false);

    if !exists {
        let password_hash = hash(&password, DEFAULT_COST).expect("Failed to hash admin password");
        sqlx::query("INSERT INTO users (email, password_hash) VALUES ($1, $2)")
            .bind(&email)
            .bind(password_hash)
            .execute(db)
            .await
            .expect("Failed to insert admin user fixture");
        println!("Fixture applied: admin user {email}");
    }
}
