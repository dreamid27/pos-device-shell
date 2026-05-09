use axum::{routing::get, Json, Router};
use serde::Serialize;

use crate::device_service::config::load_config;
use crate::device_service::types::PrinterInfo;
use crate::device_service::utils::printers::discovery::discover_printers;

#[derive(Serialize)]
struct PrintersResponse {
    printers: Vec<PrinterInfo>,
}

pub fn router() -> Router {
    Router::new().route("/printers", get(handler))
}

async fn handler() -> Json<PrintersResponse> {
    let config = load_config();
    let printers = tokio::task::spawn_blocking(move || discover_printers(&config))
        .await
        .unwrap_or_default();
    Json(PrintersResponse { printers })
}
