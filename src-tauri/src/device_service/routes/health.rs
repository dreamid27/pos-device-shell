use axum::{routing::get, Json, Router};
use serde::Serialize;

use crate::device_service::config::load_config;
use crate::device_service::types::{SubsystemHealth, SubsystemStatus};

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
    subsystems: SubsystemHealth,
}

pub fn router() -> Router {
    Router::new().route("/health", get(handler))
}

async fn handler() -> Json<HealthResponse> {
    let config = load_config();
    Json(HealthResponse {
        status: "ok",
        service: "pos-device-service",
        version: env!("CARGO_PKG_VERSION"),
        subsystems: SubsystemHealth {
            printers: SubsystemStatus {
                ok: true,
                count: config.printers.len(),
            },
            scales: SubsystemStatus {
                ok: true,
                count: config.scales.len(),
            },
            rfid: SubsystemStatus {
                ok: true,
                count: config.rfid.len(),
            },
        },
    })
}
