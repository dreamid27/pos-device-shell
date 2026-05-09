use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::device_service::handlers::scales::{
    adapter::{ScaleAdapter, SharedAdapter},
    hid_pos::HidPosScaleAdapter,
    mock::MockScaleAdapter,
    nci::NciScaleAdapter,
};
use crate::device_service::types::{Config, ScaleConfig, ScaleProtocol};
use crate::device_service::utils::scales::discovery::build_catalog;

#[derive(Default)]
pub struct ScaleRegistry {
    inner: Mutex<HashMap<String, SharedAdapter>>,
}

impl ScaleRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn get_or_build(
        &self,
        name: &str,
        config: &Config,
    ) -> anyhow::Result<SharedAdapter> {
        let mut guard = self.inner.lock().await;
        if let Some(existing) = guard.get(name) {
            return Ok(Arc::clone(existing));
        }
        let catalog = build_catalog(config);
        let cfg = catalog
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Scale not found: {name}"))?;
        let adapter = build_adapter(name.to_string(), cfg)?;
        guard.insert(name.to_string(), Arc::clone(&adapter));
        Ok(adapter)
    }

    pub async fn close_all(&self) {
        let mut guard = self.inner.lock().await;
        let drained: Vec<SharedAdapter> = guard.drain().map(|(_, v)| v).collect();
        drop(guard);
        for adapter in drained {
            adapter.close().await;
        }
    }
}

fn build_adapter(name: String, config: ScaleConfig) -> anyhow::Result<SharedAdapter> {
    match config.protocol {
        ScaleProtocol::Mock => {
            let a = MockScaleAdapter::new(name, &config);
            a.ensure_ticking();
            Ok(a as Arc<dyn ScaleAdapter>)
        }
        ScaleProtocol::Nci
        | ScaleProtocol::Cas
        | ScaleProtocol::Toledo
        | ScaleProtocol::Avery => {
            let a = NciScaleAdapter::new(name, config)?;
            a.ensure_reader()?;
            Ok(a as Arc<dyn ScaleAdapter>)
        }
        ScaleProtocol::HidPos => {
            let a = HidPosScaleAdapter::new(name, config)?;
            a.ensure_reader()?;
            Ok(a as Arc<dyn ScaleAdapter>)
        }
    }
}
