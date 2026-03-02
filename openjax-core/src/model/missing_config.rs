use anyhow::{Result, anyhow};
use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedSender;

use crate::model::client::ModelClient;
use crate::model::types::{ModelRequest, ModelResponse};

#[derive(Debug, Clone)]
pub(crate) struct MissingConfigModelClient {
    reason: String,
}

impl MissingConfigModelClient {
    pub(crate) fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

#[async_trait]
impl ModelClient for MissingConfigModelClient {
    async fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse> {
        Err(anyhow!("{}", self.reason))
    }

    async fn complete_stream(
        &self,
        _request: &ModelRequest,
        _delta_sender: Option<UnboundedSender<String>>,
    ) -> Result<ModelResponse> {
        Err(anyhow!("{}", self.reason))
    }

    fn name(&self) -> &'static str {
        "missing-model-config"
    }
}
