use crate::screen_query;
use crate::state::AppState;
use axum::extract::ws::Message;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgListener;
use std::time::Duration;
use uuid::Uuid;

pub const DEVICE_PUSH_CHANNEL: &str = "device_push";
pub const INTERACTION_UPDATE_CHANNEL: &str = "interaction_update";

#[derive(Serialize, Deserialize)]
struct PushNotification {
    device_id: Uuid,
    screen_id: Uuid,
}

#[derive(Serialize, Deserialize)]
struct InteractionPushNotification {
    device_id: Uuid,
    session_id: Uuid,
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

/// Notify every backend replica that `device_id`'s currently displayed
/// interactive slide (`session_id`) has fresh results. Only the replica
/// holding that device's WebSocket connection delivers it - see
/// [`spawn_interaction_update_listener`]. The notification carries no
/// results payload (Postgres `NOTIFY` caps out around 8000 bytes, and a
/// drawing session's stroke list can grow well past that) - the listener
/// re-fetches current results from the DB before sending, same as
/// `notify_device_push`/`run` re-fetch the screen instead of embedding it.
pub async fn notify_interaction_update(
    db: &sqlx::PgPool,
    device_id: Uuid,
    session_id: Uuid,
) -> Result<(), sqlx::Error> {
    let payload = serde_json::to_string(&InteractionPushNotification {
        device_id,
        session_id,
    })
    .unwrap_or_default();

    sqlx::query("SELECT pg_notify($1, $2)")
        .bind(INTERACTION_UPDATE_CHANNEL)
        .bind(payload)
        .execute(db)
        .await?;

    Ok(())
}

/// Runs for the lifetime of the process. Listens on `INTERACTION_UPDATE_CHANNEL`
/// and, for every notification, delivers the results to the device's
/// WebSocket if (and only if) it is connected to this replica.
pub fn spawn_interaction_update_listener(state: AppState) {
    tokio::spawn(async move {
        loop {
            if let Err(err) = run_interaction_updates(&state).await {
                eprintln!("[pubsub] interaction listener error: {err}, reconnecting in 5s");
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
}

async fn run_interaction_updates(state: &AppState) -> Result<(), sqlx::Error> {
    let mut listener = PgListener::connect_with(&state.db).await?;
    listener.listen(INTERACTION_UPDATE_CHANNEL).await?;

    loop {
        let notification = listener.recv().await?;

        let Ok(payload) =
            serde_json::from_str::<InteractionPushNotification>(notification.payload())
        else {
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

        let Ok(Some(results)) =
            crate::routes::interact::compute_results(&state.db, payload.session_id).await
        else {
            continue;
        };

        let msg_text = serde_json::json!({
            "type": "interaction_update",
            "session_id": payload.session_id,
            "results": results,
        })
        .to_string();

        let senders = state.device_senders.lock().await;
        if let Some(tx) = senders.get(&payload.device_id) {
            let _ = tx.send(Message::Text(msg_text.into()));
        }
    }
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
