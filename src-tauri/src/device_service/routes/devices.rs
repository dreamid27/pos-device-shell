use axum::{routing::get, Json, Router};

use crate::device_service::config::load_config;
use crate::device_service::types::DeviceInventory;
use crate::device_service::utils::printers::discovery::discover_printers;
use crate::device_service::utils::rfid::discovery::discover_rfid_readers;
use crate::device_service::utils::scales::discovery::discover_scales;

pub fn router() -> Router {
    Router::new().route("/devices", get(handler))
}

async fn handler() -> Json<DeviceInventory> {
    let config = load_config();
    let printers_cfg = config.clone();
    let printers = tokio::task::spawn_blocking(move || discover_printers(&printers_cfg))
        .await
        .unwrap_or_default();
    let scales_cfg = config.clone();
    let scales = tokio::task::spawn_blocking(move || discover_scales(&scales_cfg))
        .await
        .unwrap_or_default();
    let rfid = discover_rfid_readers(&config);
    Json(DeviceInventory {
        printers,
        scales,
        rfid,
    })
}
