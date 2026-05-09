use std::sync::Arc;

use tokio::sync::broadcast;

use crate::device_service::types::TagEvent;

pub type TagSender = broadcast::Sender<TagEvent>;
pub type TagReceiver = broadcast::Receiver<TagEvent>;

#[async_trait::async_trait]
pub trait RfidAdapter: Send + Sync {
    fn name(&self) -> &str;
    fn subscribe(&self) -> TagReceiver;
    async fn close(&self);
}

pub type SharedRfidAdapter = Arc<dyn RfidAdapter>;
