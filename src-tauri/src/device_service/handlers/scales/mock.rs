use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use chrono::Utc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use crate::device_service::types::{ScaleConfig, WeightReading, WeightUnit};

use super::adapter::{ScaleAdapter, WeightReceiver, WeightSender};

const CHANNEL_CAP: usize = 64;

pub struct MockScaleAdapter {
    name: String,
    unit: WeightUnit,
    tx: WeightSender,
    state: Arc<StdMutex<MockState>>,
}

#[derive(Default)]
struct MockState {
    value: f64,
    direction: f64,
    task: Option<JoinHandle<()>>,
}

impl MockScaleAdapter {
    pub fn new(name: String, config: &ScaleConfig) -> Arc<Self> {
        let (tx, _rx) = broadcast::channel(CHANNEL_CAP);
        Arc::new(Self {
            name,
            unit: config.unit.unwrap_or(WeightUnit::Kg),
            tx,
            state: Arc::new(StdMutex::new(MockState {
                value: 0.0,
                direction: 1.0,
                task: None,
            })),
        })
    }

    fn snapshot(&self, value: f64, stable: bool) -> WeightReading {
        WeightReading {
            scale: self.name.clone(),
            value,
            unit: self.unit,
            stable,
            ts: Utc::now().to_rfc3339(),
        }
    }

    pub fn ensure_ticking(self: &Arc<Self>) {
        let mut state = self.state.lock().expect("mock state poisoned");
        if state.task.is_some() {
            return;
        }
        let me = Arc::clone(self);
        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_millis(200));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                let value = {
                    let mut s = match me.state.lock() {
                        Ok(s) => s,
                        Err(_) => break,
                    };
                    s.value = ((s.value + 0.05 * s.direction) * 100.0).round() / 100.0;
                    if s.value >= 5.0 {
                        s.direction = -1.0;
                    }
                    if s.value <= 0.0 {
                        s.direction = 1.0;
                    }
                    s.value
                };
                let _ = me.tx.send(me.snapshot(value, true));
            }
        });
        state.task = Some(handle);
    }
}

#[async_trait::async_trait]
impl ScaleAdapter for MockScaleAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    async fn get_weight(&self) -> anyhow::Result<WeightReading> {
        let value = self.state.lock().expect("mock state poisoned").value;
        Ok(self.snapshot(value, true))
    }

    fn subscribe(&self) -> WeightReceiver {
        self.tx.subscribe()
    }

    async fn close(&self) {
        if let Ok(mut s) = self.state.lock() {
            if let Some(h) = s.task.take() {
                h.abort();
            }
        }
    }
}
