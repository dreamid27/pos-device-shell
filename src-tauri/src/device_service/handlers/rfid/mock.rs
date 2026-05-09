use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use chrono::Utc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use crate::device_service::types::{RfidConfig, TagEvent};

use super::adapter::{RfidAdapter, TagReceiver, TagSender};

const CHANNEL_CAP: usize = 64;

pub struct MockRfidAdapter {
    name: String,
    tx: TagSender,
    state: Arc<StdMutex<MockState>>,
}

#[derive(Default)]
struct MockState {
    counter: u64,
    task: Option<JoinHandle<()>>,
}

impl MockRfidAdapter {
    pub fn new(name: String, _config: &RfidConfig) -> Arc<Self> {
        let (tx, _rx) = broadcast::channel(CHANNEL_CAP);
        Arc::new(Self {
            name,
            tx,
            state: Arc::new(StdMutex::new(MockState::default())),
        })
    }

    pub fn ensure_ticking(self: &Arc<Self>) {
        let mut s = self.state.lock().expect("rfid mock state poisoned");
        if s.task.is_some() {
            return;
        }
        let me = Arc::clone(self);
        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(2));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                let counter = {
                    let mut s = match me.state.lock() {
                        Ok(s) => s,
                        Err(_) => break,
                    };
                    s.counter = s.counter.wrapping_add(1);
                    s.counter
                };
                let event = TagEvent {
                    reader: me.name.clone(),
                    epc: format!("E2003412{:024X}", counter),
                    tid: None,
                    rssi: Some(-55),
                    antenna: Some(1),
                    ts: Utc::now().to_rfc3339(),
                };
                let _ = me.tx.send(event);
            }
        });
        s.task = Some(handle);
    }
}

#[async_trait::async_trait]
impl RfidAdapter for MockRfidAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn subscribe(&self) -> TagReceiver {
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
