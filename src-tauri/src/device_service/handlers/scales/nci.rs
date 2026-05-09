use std::io::{ErrorKind, Read, Write};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use chrono::Utc;
use once_cell::sync::Lazy;
use regex::Regex;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use crate::device_service::transports::serial::{open_serial, SerialOptions};
use crate::device_service::types::{ScaleConfig, WeightReading, WeightUnit};

use super::adapter::{ScaleAdapter, WeightReceiver, WeightSender};

const READ_TIMEOUT_MS: u64 = 1500;
const POLL_FRAME: [u8; 3] = [0x02, 0x57, 0x0d];
const CHANNEL_CAP: usize = 64;

static FRAME_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)(?P<status>[SMOUZ])?[\s,]*(?P<sign>[-+])?\s*(?P<value>\d+(?:\.\d+)?)\s*(?P<unit>kg|lb|oz|g)",
    )
    .expect("nci frame regex")
});

pub struct NciScaleAdapter {
    name: String,
    config: ScaleConfig,
    default_unit: WeightUnit,
    tx: WeightSender,
    state: Arc<StdMutex<NciState>>,
}

#[derive(Default)]
struct NciState {
    rx_buffer: String,
    latest: Option<WeightReading>,
    reader_task: Option<JoinHandle<()>>,
}

impl NciScaleAdapter {
    pub fn new(name: String, config: ScaleConfig) -> anyhow::Result<Arc<Self>> {
        if config.path.is_none() {
            anyhow::bail!("Scale {name}: serial 'path' is required for NCI protocol");
        }
        let default_unit = config.unit.unwrap_or(WeightUnit::Kg);
        let (tx, _rx) = broadcast::channel(CHANNEL_CAP);
        Ok(Arc::new(Self {
            name,
            config,
            default_unit,
            tx,
            state: Arc::new(StdMutex::new(NciState::default())),
        }))
    }

    pub fn ensure_reader(self: &Arc<Self>) -> anyhow::Result<()> {
        let mut state = self.state.lock().expect("nci state poisoned");
        if state.reader_task.is_some() {
            return Ok(());
        }

        let path = self
            .config
            .path
            .clone()
            .ok_or_else(|| anyhow::anyhow!("missing serial path"))?;
        let baud = self.config.baud.unwrap_or(9600);

        let me = Arc::clone(self);
        let handle = tokio::task::spawn_blocking(move || me.read_loop(path, baud));
        state.reader_task = Some(handle);
        Ok(())
    }

    fn read_loop(self: Arc<Self>, path: String, baud: u32) {
        let mut port = match open_serial(&SerialOptions { path, baud }) {
            Ok(p) => p,
            Err(e) => {
                log::warn!("[scale {}] open failed: {e}", self.name);
                return;
            }
        };
        let _ = port.write_all(&POLL_FRAME);

        let mut buf = [0u8; 256];
        loop {
            match port.read(&mut buf) {
                Ok(0) => continue,
                Ok(n) => {
                    let chunk = String::from_utf8_lossy(&buf[..n]);
                    self.feed(&chunk);
                }
                Err(e) if e.kind() == ErrorKind::TimedOut => continue,
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => {
                    log::warn!("[scale {}] read error: {e}", self.name);
                    std::thread::sleep(Duration::from_millis(250));
                }
            }
        }
    }

    fn feed(&self, chunk: &str) {
        let mut state = self.state.lock().expect("nci state poisoned");
        state.rx_buffer.push_str(chunk);
        state.rx_buffer = state.rx_buffer.replace('\0', "").replace('\x03', "\n");

        let buffer_snapshot = state.rx_buffer.clone();
        let Some(caps) = FRAME_REGEX.captures(&buffer_snapshot) else {
            return;
        };

        let sign = caps.name("sign").map(|m| m.as_str()).unwrap_or("+");
        let value_str = caps.name("value").map(|m| m.as_str()).unwrap_or("0");
        let unit = caps
            .name("unit")
            .map(|m| normalize_unit(m.as_str(), self.default_unit))
            .unwrap_or(self.default_unit);
        let in_motion = caps
            .name("status")
            .map(|m| m.as_str().eq_ignore_ascii_case("M"))
            .unwrap_or(false);
        let value: f64 = value_str.parse().unwrap_or(0.0);
        let value = if sign == "-" { -value } else { value };

        let reading = WeightReading {
            scale: self.name.clone(),
            value,
            unit,
            stable: !in_motion,
            ts: Utc::now().to_rfc3339(),
        };

        let mat_end = caps.get(0).map(|m| m.end()).unwrap_or(0);
        state.rx_buffer = state.rx_buffer.split_at(mat_end).1.to_string();
        state.latest = Some(reading.clone());
        drop(state);

        let _ = self.tx.send(reading);
    }
}

fn normalize_unit(raw: &str, fallback: WeightUnit) -> WeightUnit {
    match raw.to_lowercase().as_str() {
        "kg" => WeightUnit::Kg,
        "g" => WeightUnit::G,
        "lb" => WeightUnit::Lb,
        "oz" => WeightUnit::Oz,
        _ => fallback,
    }
}

#[async_trait::async_trait]
impl ScaleAdapter for NciScaleAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    async fn get_weight(&self) -> anyhow::Result<WeightReading> {
        let mut rx = self.tx.subscribe();
        match tokio::time::timeout(Duration::from_millis(READ_TIMEOUT_MS), rx.recv()).await {
            Ok(Ok(r)) => Ok(r),
            _ => {
                let s = self.state.lock().expect("nci state poisoned");
                s.latest.clone().ok_or_else(|| {
                    anyhow::anyhow!(
                        "Scale {} did not respond within {READ_TIMEOUT_MS}ms",
                        self.name
                    )
                })
            }
        }
    }

    fn subscribe(&self) -> WeightReceiver {
        self.tx.subscribe()
    }

    async fn close(&self) {
        let mut s = self.state.lock().expect("nci state poisoned");
        if let Some(h) = s.reader_task.take() {
            h.abort();
        }
    }
}
