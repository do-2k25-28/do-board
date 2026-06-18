use shared::Device;
use sqlx::PgPool;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

pub type DeviceStore = Arc<RwLock<HashMap<String, Device>>>;

#[derive(Clone)]
pub struct AppState {
    pub devices: DeviceStore,
    pub db: PgPool,
    pub jwt_secret: String,
}
