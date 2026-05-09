use crate::device_service::types::{Config, DeviceStatus, RfidInfo};

/// List configured RFID readers with placeholder status.
/// Real status (LLRP keepalive, serial handshake) will be wired when adapters land.
pub fn discover_rfid_readers(config: &Config) -> Vec<RfidInfo> {
    config
        .rfid
        .iter()
        .map(|(name, cfg)| RfidInfo {
            name: name.clone(),
            protocol: cfg.protocol,
            transport: cfg.transport,
            status: DeviceStatus::Ready,
        })
        .collect()
}
