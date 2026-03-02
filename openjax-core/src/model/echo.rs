use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedSender;

use crate::model::client::ModelClient;
use crate::model::types::{ModelRequest, ModelResponse};

#[derive(Debug, Default)]
pub(crate) struct EchoModelClient;

#[async_trait]
impl ModelClient for EchoModelClient {
    async fn complete(&self, request: &ModelRequest) -> Result<ModelResponse> {
        Ok(ModelResponse {
            text: format!("[Echo fallback] {}", request.user_input),
            ..ModelResponse::default()
        })
    }

    async fn complete_stream(
        &self,
        request: &ModelRequest,
        delta_sender: Option<UnboundedSender<String>>,
    ) -> Result<ModelResponse> {
        let text = format!("[Echo fallback] {}", request.user_input);
        if let Some(sender) = delta_sender {
            let _ = sender.send(text.clone());
        }
        Ok(ModelResponse {
            text,
            ..ModelResponse::default()
        })
    }

    fn name(&self) -> &'static str {
        "echo"
    }
}
