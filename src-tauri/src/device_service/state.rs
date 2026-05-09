use std::sync::Arc;

use crate::device_service::utils::rfid::registry::RfidRegistry;
use crate::device_service::utils::scales::registry::ScaleRegistry;

#[derive(Clone)]
pub struct AppState {
    pub scales: Arc<ScaleRegistry>,
    pub rfid: Arc<RfidRegistry>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            scales: Arc::new(ScaleRegistry::new()),
            rfid: Arc::new(RfidRegistry::new()),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
