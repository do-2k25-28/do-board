use crate::auth::AuthUser;
use crate::pubsub;
use crate::screen_query;
use crate::state::{AppState, DeviceSenders};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        ConnectInfo, Path, State,
    },
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use futures::{SinkExt, StreamExt};
use shared::{Device, PushScreenRequest, SaveDeviceRequest};
use std::net::SocketAddr;
use tokio::sync::mpsc;
use uuid::Uuid;

// Fixed, arbitrary namespace so a device's UUID is deterministic (same
// ip+user-agent always maps to the same id) without needing any client-side
// storage or cookie.
const DEVICE_NAMESPACE: Uuid = Uuid::from_u128(0x2e1a8b3a_0f0a_4e9b_9d0e_6a2c9f6b7a10);

fn device_fingerprint(ip: &str, user_agent: &str) -> Uuid {
    Uuid::new_v5(&DEVICE_NAMESPACE, format!("{ip}|{user_agent}").as_bytes())
}

fn parse_device_id(id: &str) -> Result<Uuid, (StatusCode, &'static str)> {
    Uuid::parse_str(id).map_err(|_| (StatusCode::BAD_REQUEST, "Invalid device ID"))
}

#[derive(sqlx::FromRow)]
struct DeviceRow {
    id: Uuid,
    ip: String,
    browser: String,
    os: String,
    online: bool,
    connected_at: chrono::DateTime<chrono::Utc>,
    last_seen: chrono::DateTime<chrono::Utc>,
    name: Option<String>,
    saved: bool,
}

impl DeviceRow {
    fn into_device(self) -> Device {
        Device {
            id: self.id.to_string(),
            ip: self.ip,
            browser: self.browser,
            os: self.os,
            online: self.online,
            connected_at: self
                .connected_at
                .format("%Y-%m-%d %H:%M:%S UTC")
                .to_string(),
            last_seen: self.last_seen.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            name: self.name,
            saved: self.saved,
        }
    }
}

pub async fn list_devices(State(state): State<AppState>) -> Json<Vec<Device>> {
    let rows: Vec<DeviceRow> = sqlx::query_as(
        "SELECT id, ip, browser, os, online, connected_at, last_seen, name, saved FROM devices",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let mut list: Vec<Device> = rows.into_iter().map(DeviceRow::into_device).collect();

    list.sort_by(|a, b| {
        b.online
            .cmp(&a.online)
            .then_with(|| a.connected_at.cmp(&b.connected_at))
    });
    Json(list)
}

pub async fn save_device(
    _auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<SaveDeviceRequest>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    let device_id = parse_device_id(&id)?;

    let result = sqlx::query("UPDATE devices SET name = $1, saved = TRUE WHERE id = $2")
        .bind(&req.name)
        .bind(device_id)
        .execute(&state.db)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Device not found"));
    }

    Ok(StatusCode::OK)
}

pub async fn push_screen(
    _auth: AuthUser,
    State(state): State<AppState>,
    Path(device_id): Path<String>,
    Json(req): Json<PushScreenRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let device_id = parse_device_id(&device_id).map_err(|(s, m)| (s, m.to_string()))?;
    let screen_uuid = Uuid::parse_str(&req.screen_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid screen ID".into()))?;

    screen_query::fetch_screen(&state.db, screen_uuid)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error".into()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Screen not found".into()))?;

    let online: Option<bool> = sqlx::query_scalar("SELECT online FROM devices WHERE id = $1")
        .bind(device_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error".into()))?;

    if online != Some(true) {
        return Err((StatusCode::NOT_FOUND, "Device not connected".into()));
    }

    sqlx::query("UPDATE devices SET current_screen_id = $1 WHERE id = $2")
        .bind(screen_uuid)
        .bind(device_id)
        .execute(&state.db)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error".into()))?;

    // Fan out to every replica - whichever one actually holds this device's
    // WebSocket connection will deliver it. See crate::pubsub.
    pubsub::notify_device_push(&state.db, device_id, screen_uuid)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error".into()))?;

    Ok(StatusCode::OK)
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("Unknown")
        .to_string();

    let ip = addr.ip().to_string();
    let fingerprint = device_fingerprint(&ip, &user_agent);
    let browser = parse_browser(&user_agent);
    let os = parse_os(&user_agent);

    // Devices always reset to the current default screen on (re)connect.
    let default_screen = screen_query::fetch_default_screen(&state.db)
        .await
        .ok()
        .flatten();
    let default_screen_uuid = default_screen
        .as_ref()
        .and_then(|s| Uuid::parse_str(&s.id).ok());

    let _ = sqlx::query(
        "INSERT INTO devices (id, ip, browser, os, online, connected_at, last_seen, current_screen_id)
         VALUES ($1, $2, $3, $4, TRUE, NOW(), NOW(), $5)
         ON CONFLICT (id) DO UPDATE SET
             ip = EXCLUDED.ip, browser = EXCLUDED.browser, os = EXCLUDED.os,
             online = TRUE, last_seen = NOW(), current_screen_id = EXCLUDED.current_screen_id",
    )
    .bind(fingerprint)
    .bind(&ip)
    .bind(&browser)
    .bind(&os)
    .bind(default_screen_uuid)
    .execute(&state.db)
    .await;

    let initial_msg = default_screen
        .as_ref()
        .map(screen_query::set_screen_message);
    let db = state.db.clone();

    ws.on_upgrade(move |socket| {
        handle_socket(socket, db, state.device_senders, fingerprint, initial_msg)
    })
}

async fn handle_socket(
    socket: WebSocket,
    db: sqlx::PgPool,
    device_senders: DeviceSenders,
    fingerprint: Uuid,
    initial_msg: Option<String>,
) {
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    {
        let mut senders = device_senders.lock().await;
        senders.insert(fingerprint, tx.clone());
    }

    // Send default screen immediately on connect
    if let Some(msg) = initial_msg {
        let _ = tx.send(Message::Text(msg.into()));
    }

    let (mut sink, mut stream) = socket.split();

    // Forward queued messages → device
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sink.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Read from device
    while let Some(result) = stream.next().await {
        match result {
            Ok(msg) => {
                if matches!(msg, Message::Close(_)) {
                    break;
                }
                let _ = sqlx::query("UPDATE devices SET last_seen = NOW() WHERE id = $1")
                    .bind(fingerprint)
                    .execute(&db)
                    .await;
            }
            Err(_) => break,
        }
    }

    send_task.abort();

    {
        let mut senders = device_senders.lock().await;
        senders.remove(&fingerprint);
    }

    let _ = sqlx::query("UPDATE devices SET online = FALSE, last_seen = NOW() WHERE id = $1")
        .bind(fingerprint)
        .execute(&db)
        .await;
}

fn parse_browser(ua: &str) -> String {
    if ua.contains("Edg/") || ua.contains("Edge/") {
        "Microsoft Edge".into()
    } else if ua.contains("OPR/") || ua.contains("Opera/") {
        "Opera".into()
    } else if ua.contains("Chrome/") {
        "Chrome".into()
    } else if ua.contains("Firefox/") {
        "Firefox".into()
    } else if ua.contains("Safari/") {
        "Safari".into()
    } else {
        "Unknown".into()
    }
}

fn parse_os(ua: &str) -> String {
    if ua.contains("iPhone") || ua.contains("iPad") {
        "iOS".into()
    } else if ua.contains("Android") {
        "Android".into()
    } else if ua.contains("Windows NT 10.0") {
        "Windows 10/11".into()
    } else if ua.contains("Windows NT") {
        "Windows".into()
    } else if ua.contains("Mac OS X") {
        "macOS".into()
    } else if ua.contains("Linux") {
        "Linux".into()
    } else {
        "Unknown".into()
    }
}
