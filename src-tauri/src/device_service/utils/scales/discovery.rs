use crate::device_service::transports::hid::find_hid_pos_scales;
use crate::device_service::types::{
    Config, DeviceStatus, ScaleConfig, ScaleInfo, ScaleProtocol, ScaleTransport, WeightUnit,
};

fn to_hex(n: u16) -> String {
    format!("0x{:04x}", n)
}

/// Build catalog of scales available right now: HID-POS auto-detection + config overrides.
pub fn discover_scales(config: &Config) -> Vec<ScaleInfo> {
    let mut out: Vec<ScaleInfo> = Vec::new();

    for d in find_hid_pos_scales() {
        let name = format!("hid:{}-{}", to_hex(d.vendor_id), to_hex(d.product_id));
        out.push(ScaleInfo {
            name,
            protocol: ScaleProtocol::HidPos,
            transport: ScaleTransport::UsbHid,
            status: DeviceStatus::Ready,
        });
    }

    for (name, cfg) in &config.scales {
        if out.iter().any(|s| &s.name == name) {
            continue;
        }
        out.push(ScaleInfo {
            name: name.clone(),
            protocol: cfg.protocol,
            transport: cfg.transport,
            status: DeviceStatus::Ready,
        });
    }

    out
}

/// Merge HID-detected + config-defined scales into a name-keyed catalog.
/// Used by the registry when it needs to instantiate an adapter for a name
/// that may not be in the config file (auto-detected HID).
pub fn build_catalog(config: &Config) -> std::collections::HashMap<String, ScaleConfig> {
    let mut out = std::collections::HashMap::new();

    for d in find_hid_pos_scales() {
        let name = format!("hid:{}-{}", to_hex(d.vendor_id), to_hex(d.product_id));
        out.insert(
            name,
            ScaleConfig {
                protocol: ScaleProtocol::HidPos,
                transport: ScaleTransport::UsbHid,
                path: None,
                baud: None,
                vendor_id: Some(to_hex(d.vendor_id)),
                product_id: Some(to_hex(d.product_id)),
                unit: Some(WeightUnit::Kg),
            },
        );
    }

    for (name, cfg) in &config.scales {
        out.insert(name.clone(), cfg.clone());
    }
    out
}
