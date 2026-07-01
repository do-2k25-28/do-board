use crate::screen_query;
use crate::state::AppState;
use axum::extract::ws::Message;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgListener;
use std::time::Duration;
use uuid::Uuid;

pub const DEVICE_PUSH_CHANNEL: &str = "device_push";

#[derive(Serialize, Deserialize)]
struct PushNotification {
    device_id: Uuid,
    screen_id: Uuid,
}

/// Notify every backend replica that `device_id` should now display
/// `screen_id`. Only the replica currently holding that device's WebSocket
/// connection (tracked in its own in-memory `device_senders`) will actually
/// deliver the message - see [`spawn_device_push_listener`].
pub async fn notify_device_push(
    db: &sqlx::PgPool,
    device_id: Uuid,
    screen_id: Uuid,
) -> Result<(), sqlx::Error> {
    let payload = serde_json::to_string(&PushNotification {
        device_id,
        screen_id,
    })
    .unwrap_or_default();

    sqlx::query("SELECT pg_notify($1, $2)")
        .bind(DEVICE_PUSH_CHANNEL)
        .bind(payload)
        .execute(db)
        .await?;

    Ok(())
}

/// Runs for the lifetime of the process. Listens on `DEVICE_PUSH_CHANNEL` and,
/// for every notification, delivers the target screen to the device's
/// WebSocket if (and only if) it is connected to this replica.
pub fn spawn_device_push_listener(state: AppState) {
    tokio::spawn(async move {
        loop {
            if let Err(err) = run(&state).await {
                eprintln!("[pubsub] listener error: {err}, reconnecting in 5s");
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
}

async fn run(state: &AppState) -> Result<(), sqlx::Error> {
    let mut listener = PgListener::connect_with(&state.db).await?;
    listener.listen(DEVICE_PUSH_CHANNEL).await?;

    loop {
        let notification = listener.recv().await?;

        let Ok(payload) = serde_json::from_str::<PushNotification>(notification.payload()) else {
            continue;
        };

        // Cheap check first: skip the DB round-trip entirely if this replica
        // doesn't hold the device's connection.
        let is_local = state
            .device_senders
            .lock()
            .await
            .contains_key(&payload.device_id);
        if !is_local {
            continue;
        }

        let Ok(Some(screen)) = screen_query::fetch_screen(&state.db, payload.screen_id).await
        else {
            continue;
        };
        let msg_text = screen_query::set_screen_message(&screen);

        let senders = state.device_senders.lock().await;
        if let Some(tx) = senders.get(&payload.device_id) {
            let _ = tx.send(Message::Text(msg_text.into()));
        }
    }
}
