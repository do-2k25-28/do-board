use crate::auth::AuthUser;
use crate::state::{AppState, DeviceStore};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        ConnectInfo, Path, State,
    },
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use shared::{Device, SaveDeviceRequest};
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
    net::SocketAddr,
};

#[derive(sqlx::FromRow)]
struct SavedDeviceRow {
    id: String,
    name: String,
    ip: String,
    browser: String,
    os: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_devices(State(state): State<AppState>) -> Json<Vec<Device>> {
    let saved: Vec<SavedDeviceRow> =
        sqlx::query_as("SELECT id, name, ip, browser, os, created_at FROM saved_devices")
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();

    let saved_map: HashMap<String, SavedDeviceRow> =
        saved.into_iter().map(|r| (r.id.clone(), r)).collect();

    let mut result: HashMap<String, Device> = {
        let store = state.devices.read().unwrap();
        store
            .iter()
            .map(|(k, v)| {
                let mut d = v.clone();
                if let Some(row) = saved_map.get(k) {
                    d.saved = true;
                    d.name = Some(row.name.clone());
                }
                (k.clone(), d)
            })
            .collect()
    };

    for (id, row) in &saved_map {
        if !result.contains_key(id) {
            let ts = row.created_at.format("%Y-%m-%d %H:%M:%S UTC").to_string();
            result.insert(
                id.clone(),
                Device {
                    id: id.clone(),
                    ip: row.ip.clone(),
                    browser: row.browser.clone(),
                    os: row.os.clone(),
                    online: false,
                    connected_at: ts.clone(),
                    last_seen: ts,
                    name: Some(row.name.clone()),
                    saved: true,
                },
            );
        }
    }

    let mut list: Vec<Device> = result.into_values().collect();
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
    let (ip, browser, os) = {
        let store = state.devices.read().unwrap();
        if let Some(d) = store.get(&id) {
            (d.ip.clone(), d.browser.clone(), d.os.clone())
        } else {
            (String::new(), String::new(), String::new())
        }
    };

    sqlx::query(
        "INSERT INTO saved_devices (id, name, ip, browser, os)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, ip = EXCLUDED.ip,
             browser = EXCLUDED.browser, os = EXCLUDED.os",
    )
    .bind(&id)
    .bind(&req.name)
    .bind(&ip)
    .bind(&browser)
    .bind(&os)
    .execute(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

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
    let now = now_str();

    {
        let mut store = state.devices.write().unwrap();
        if let Some(existing) = store.get_mut(&fingerprint) {
            existing.online = true;
            existing.last_seen = now;
        } else {
            store.insert(
                fingerprint.clone(),
                Device {
                    id: fingerprint.clone(),
                    ip,
                    browser: parse_browser(&user_agent),
                    os: parse_os(&user_agent),
                    online: true,
                    connected_at: now.clone(),
                    last_seen: now,
                    name: None,
                    saved: false,
                },
            );
        }
    }

    ws.on_upgrade(move |socket| handle_socket(socket, state.devices, fingerprint))
}

async fn handle_socket(mut socket: WebSocket, devices: DeviceStore, fingerprint: String) {
    while let Some(Ok(msg)) = socket.recv().await {
        if matches!(msg, Message::Close(_)) {
            break;
        }
        if let Ok(mut store) = devices.write() {
            if let Some(device) = store.get_mut(&fingerprint) {
                device.last_seen = now_str();
            }
        }
    }
    if let Ok(mut store) = devices.write() {
        if let Some(device) = store.get_mut(&fingerprint) {
            device.online = false;
            device.last_seen = now_str();
        }
    }
}

fn device_fingerprint(ip: &str, user_agent: &str) -> String {
    let mut h = DefaultHasher::new();
    ip.hash(&mut h);
    user_agent.hash(&mut h);
    format!("{:x}", h.finish())
}

fn now_str() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%d %H:%M:%S UTC")
        .to_string()
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
