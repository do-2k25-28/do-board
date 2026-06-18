use crate::state::{AppState, DeviceStore};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        ConnectInfo, State,
    },
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use shared::Device;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    net::SocketAddr,
};

pub async fn list_devices(State(state): State<AppState>) -> Json<Vec<Device>> {
    let mut list: Vec<Device> = state.devices.read().unwrap().values().cloned().collect();
    list.sort_by(|a, b| {
        b.online
            .cmp(&a.online)
            .then_with(|| a.connected_at.cmp(&b.connected_at))
    });
    Json(list)
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
