use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use chrono::Utc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use crate::device_service::transports::hid;
use crate::device_service::types::{ScaleConfig, WeightReading, WeightUnit};

use super::adapter::{ScaleAdapter, WeightReceiver, WeightSender};

const STATUS_STABLE_WEIGHT: u8 = 1;
const READ_TIMEOUT_MS: u64 = 1500;
const CHANNEL_CAP: usize = 64;

pub struct HidPosScaleAdapter {
    name: String,
    config: ScaleConfig,
    fallback_unit: WeightUnit,
    tx: WeightSender,
    state: Arc<StdMutex<HidState>>,
}

#[derive(Default)]
struct HidState {
    latest: Option<WeightReading>,
    reader_task: Option<JoinHandle<()>>,
}

struct ParsedReport {
    status: u8,
    unit: WeightUnit,
    value: f64,
}

impl HidPosScaleAdapter {
    pub fn new(name: String, config: ScaleConfig) -> anyhow::Result<Arc<Self>> {
        if config.vendor_id.is_none() || config.product_id.is_none() {
            anyhow::bail!("Scale {name}: 'vendorId' and 'productId' required for HID-POS");
        }
        let fallback_unit = config.unit.unwrap_or(WeightUnit::Kg);
        let (tx, _rx) = broadcast::channel(CHANNEL_CAP);
        Ok(Arc::new(Self {
            name,
            config,
            fallback_unit,
            tx,
            state: Arc::new(StdMutex::new(HidState::default())),
        }))
    }

    pub fn ensure_reader(self: &Arc<Self>) -> anyhow::Result<()> {
        let mut state = self.state.lock().expect("hid state poisoned");
        if state.reader_task.is_some() {
            return Ok(());
        }
        let vid = parse_hex_u16(&self.config.vendor_id)?;
        let pid = parse_hex_u16(&self.config.product_id)?;
        let me = Arc::clone(self);
        let handle = tokio::task::spawn_blocking(move || me.read_loop(vid, pid));
        state.reader_task = Some(handle);
        Ok(())
    }

    fn read_loop(self: Arc<Self>, vid: u16, pid: u16) {
        let device = match hid::open(vid, pid) {
            Ok(d) => d,
            Err(e) => {
                log::warn!("[scale {}] HID open failed: {e}", self.name);
                return;
            }
        };
        let mut buf = [0u8; 64];
        loop {
            match device.read_timeout(&mut buf, 250) {
                Ok(0) => continue,
                Ok(n) => {
                    if let Some(parsed) = parse_hid_pos_report(&buf[..n], self.fallback_unit) {
                        let reading = WeightReading {
                            scale: self.name.clone(),
                            value: parsed.value,
                            unit: parsed.unit,
                            stable: parsed.status == STATUS_STABLE_WEIGHT,
                            ts: Utc::now().to_rfc3339(),
                        };
                        if let Ok(mut s) = self.state.lock() {
                            s.latest = Some(reading.clone());
                        }
                        let _ = self.tx.send(reading);
                    }
                }
                Err(e) => {
                    log::warn!("[scale {}] HID read error: {e}", self.name);
                    std::thread::sleep(Duration::from_millis(250));
                }
            }
        }
    }
}

fn parse_hex_u16(input: &Option<String>) -> anyhow::Result<u16> {
    let s = input
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("missing hex value"))?;
    let trimmed = s.trim_start_matches("0x").trim_start_matches("0X");
    u16::from_str_radix(trimmed, 16).map_err(|e| anyhow::anyhow!("invalid hex {s}: {e}"))
}

fn unit_from_code(code: u8, fallback: WeightUnit) -> WeightUnit {
    match code {
        2 | 3 => WeightUnit::G,
        4 => WeightUnit::Kg,
        11 => WeightUnit::Oz,
        12 => WeightUnit::Lb,
        _ => fallback,
    }
}

fn parse_hid_pos_report(data: &[u8], fallback: WeightUnit) -> Option<ParsedReport> {
    if data.len() < 6 {
        return None;
    }
    let status = data[1];
    let unit = unit_from_code(data[2], fallback);
    let exponent = data[3] as i8;
    let raw = u16::from_le_bytes([data[4], data[5]]);
    let value = (raw as f64) * 10f64.powi(exponent as i32);
    Some(ParsedReport {
        status,
        unit,
        value,
    })
}

#[async_trait::async_trait]
impl ScaleAdapter for HidPosScaleAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    async fn get_weight(&self) -> anyhow::Result<WeightReading> {
        if let Ok(s) = self.state.lock() {
            if let Some(r) = s.latest.clone() {
                return Ok(r);
            }
        }
        let mut rx = self.tx.subscribe();
        match tokio::time::timeout(Duration::from_millis(READ_TIMEOUT_MS), rx.recv()).await {
            Ok(Ok(r)) => Ok(r),
            _ => anyhow::bail!(
                "Scale {} did not emit a HID report within {READ_TIMEOUT_MS}ms",
                self.name
            ),
        }
    }

    fn subscribe(&self) -> WeightReceiver {
        self.tx.subscribe()
    }

    async fn close(&self) {
        if let Ok(mut s) = self.state.lock() {
            if let Some(h) = s.reader_task.take() {
                h.abort();
            }
        }
    }
}
