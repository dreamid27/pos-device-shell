use std::sync::Mutex;

use hidapi::{DeviceInfo, HidApi, HidDevice};
use once_cell::sync::Lazy;

pub const HID_POS_SCALE_USAGE_PAGE: u16 = 0x8d;

static HIDAPI: Lazy<Mutex<Option<HidApi>>> = Lazy::new(|| Mutex::new(HidApi::new().ok()));

#[derive(Debug, Clone)]
pub struct HidDeviceSummary {
    pub vendor_id: u16,
    pub product_id: u16,
    pub usage_page: u16,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
}

pub fn list_devices() -> Vec<HidDeviceSummary> {
    let mut guard = match HIDAPI.lock() {
        Ok(g) => g,
        Err(_) => return Vec::new(),
    };
    let api = match guard.as_mut() {
        Some(api) => api,
        None => return Vec::new(),
    };
    if api.refresh_devices().is_err() {
        return Vec::new();
    }
    api.device_list().map(summary_from).collect()
}

pub fn find_hid_pos_scales() -> Vec<HidDeviceSummary> {
    list_devices()
        .into_iter()
        .filter(|d| d.usage_page == HID_POS_SCALE_USAGE_PAGE)
        .collect()
}

pub fn open(vendor_id: u16, product_id: u16) -> hidapi::HidResult<HidDevice> {
    let mut guard = HIDAPI
        .lock()
        .map_err(|_| hidapi::HidError::InitializationError)?;
    let api = guard.as_mut().ok_or(hidapi::HidError::InitializationError)?;
    api.refresh_devices().ok();
    api.open(vendor_id, product_id)
}

fn summary_from(info: &DeviceInfo) -> HidDeviceSummary {
    HidDeviceSummary {
        vendor_id: info.vendor_id(),
        product_id: info.product_id(),
        usage_page: info.usage_page(),
        manufacturer: info.manufacturer_string().map(|s| s.to_string()),
        product: info.product_string().map(|s| s.to_string()),
    }
}
