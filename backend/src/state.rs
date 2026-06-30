use axum::extract::ws::Message;
use shared::Device;
use sqlx::PgPool;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::{mpsc::UnboundedSender, Mutex, RwLock};

pub type DeviceStore = Arc<std::sync::RwLock<HashMap<String, Device>>>;
pub type DeviceSenders = Arc<Mutex<HashMap<String, UnboundedSender<Message>>>>;
/// Maps device_id → screen_id currently displayed on that device.
pub type DeviceScreens = Arc<Mutex<HashMap<String, String>>>;
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
    pub devices: DeviceStore,
    pub device_senders: DeviceSenders,
    pub device_screens: DeviceScreens,
    pub db: PgPool,
    pub jwt_secret: String,
    pub gtfs: GtfsStore,
}
