use std::sync::Arc;

use tokio::sync::broadcast;

use crate::device_service::types::WeightReading;

pub type WeightSender = broadcast::Sender<WeightReading>;
pub type WeightReceiver = broadcast::Receiver<WeightReading>;

#[async_trait::async_trait]
pub trait ScaleAdapter: Send + Sync {
    fn name(&self) -> &str;
    async fn get_weight(&self) -> anyhow::Result<WeightReading>;
    fn subscribe(&self) -> WeightReceiver;
    async fn close(&self);
}

pub type SharedAdapter = Arc<dyn ScaleAdapter>;
