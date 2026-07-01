use axum::extract::ws::Message;
use sqlx::PgPool;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::{mpsc::UnboundedSender, Mutex, RwLock};
use uuid::Uuid;

/// Live WebSocket senders for devices connected *to this replica*. Presence
/// (online/last_seen/current_screen) is authoritative in Postgres (`devices`
/// table) so it stays consistent across replicas - only the actual socket
/// handle has to live in-process, per pod. See `crate::pubsub` for how a
/// push request on one replica reaches a device connected to another.
pub type DeviceSenders = Arc<Mutex<HashMap<Uuid, UnboundedSender<Message>>>>;
/// GTFS static data cache (refreshed daily). Uses tokio RwLock so it is safe
/// to hold across await points without making futures !Send.
pub type GtfsStore = Arc<RwLock<Option<GtfsCache>>>;

pub struct RouteInfo {
    pub short_name: String,
    pub color: String, // 6-char hex, no leading #
    pub text_color: String,
    pub route_type: u16, // 0=tram, 3=bus
}

pub struct TripInfo {
    pub route_id: String,
    pub headsign: String,
    pub service_id: String,
}

pub struct StopTimeDep {
    pub trip_id: String,
    pub dep_secs: u32, // seconds since local midnight
}

/// In-memory parsed static GTFS data (refreshed daily).
pub struct GtfsCache {
    pub stops: HashMap<String, String>,     // stop_id → stop_name
    pub trips: HashMap<String, TripInfo>,   // trip_id → TripInfo
    pub routes: HashMap<String, RouteInfo>, // route_id → RouteInfo
    pub stop_times: HashMap<String, Vec<StopTimeDep>>, // stop_id → departures
    pub active_services: HashSet<String>,   // service_ids running today
}

#[derive(Clone)]
pub struct AppState {
    pub device_senders: DeviceSenders,
    pub db: PgPool,
    pub jwt_secret: String,
    /// Whether to mark the auth cookie `Secure`. Only safe to enable when the
    /// app is actually served over HTTPS (e.g. behind the Helm/Traefik
    /// ingress) - browsers refuse `Secure` cookies over plain HTTP.
    pub cookie_secure: bool,
    pub gtfs: GtfsStore,
}
