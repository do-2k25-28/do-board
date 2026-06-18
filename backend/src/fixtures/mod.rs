mod users;

use sqlx::PgPool;

pub async fn run(db: &PgPool) {
    users::run(db).await;
}
