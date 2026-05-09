use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::device_service::handlers::rfid::{
    adapter::{RfidAdapter, SharedRfidAdapter},
    llrp::LlrpRfidAdapter,
    mock::MockRfidAdapter,
};
use crate::device_service::types::{Config, RfidConfig, RfidProtocol};

#[derive(Default)]
pub struct RfidRegistry {
    inner: Mutex<HashMap<String, SharedRfidAdapter>>,
}

impl RfidRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn get_or_build(
        &self,
        name: &str,
        config: &Config,
    ) -> anyhow::Result<SharedRfidAdapter> {
        let mut guard = self.inner.lock().await;
        if let Some(existing) = guard.get(name) {
            return Ok(Arc::clone(existing));
        }
        let cfg = config
            .rfid
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("RFID reader not found: {name}"))?;
        let adapter = build_adapter(name.to_string(), cfg)?;
        guard.insert(name.to_string(), Arc::clone(&adapter));
        Ok(adapter)
    }

    pub async fn close_all(&self) {
        let mut guard = self.inner.lock().await;
        let drained: Vec<SharedRfidAdapter> = guard.drain().map(|(_, v)| v).collect();
        drop(guard);
        for adapter in drained {
            adapter.close().await;
        }
    }
}

fn build_adapter(name: String, config: RfidConfig) -> anyhow::Result<SharedRfidAdapter> {
    match config.protocol {
        RfidProtocol::Mock => {
            let a = MockRfidAdapter::new(name, &config);
            a.ensure_ticking();
            Ok(a as Arc<dyn RfidAdapter>)
        }
        RfidProtocol::Llrp => {
            let a = LlrpRfidAdapter::new(name, config)?;
            a.ensure_reader()?;
            Ok(a as Arc<dyn RfidAdapter>)
        }
        RfidProtocol::HidKeyboard => {
            anyhow::bail!("hid-keyboard readers run in browser, no service stream")
        }
        RfidProtocol::SerialProprietary => {
            anyhow::bail!("serial-proprietary RFID adapter not yet implemented")
        }
    }
}
