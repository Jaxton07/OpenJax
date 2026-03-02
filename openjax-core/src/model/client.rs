use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedSender;

use crate::model::types::{CapabilityFlags, ModelRequest, ModelResponse};

#[async_trait]
pub trait ModelClient: Send + Sync {
    async fn complete(&self, request: &ModelRequest) -> Result<ModelResponse>;

    async fn complete_stream(
        &self,
        request: &ModelRequest,
        delta_sender: Option<UnboundedSender<String>>,
    ) -> Result<ModelResponse>;

    fn name(&self) -> &'static str;
}

#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    async fn complete(&self, request: &ModelRequest) -> Result<ModelResponse>;

    async fn complete_stream(
        &self,
        request: &ModelRequest,
        delta_sender: Option<UnboundedSender<String>>,
    ) -> Result<ModelResponse>;

    fn backend_name(&self) -> &'static str;
    fn model_id(&self) -> &str;
    fn provider(&self) -> &str;
    fn protocol(&self) -> &str;
    fn capabilities(&self) -> CapabilityFlags;
}
