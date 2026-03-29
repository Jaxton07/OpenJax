use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedSender;

use crate::model::client::ModelClient;
use crate::model::types::{
    AssistantContentBlock, ModelRequest, ModelResponse, StopReason, StreamDelta,
};

#[derive(Debug, Default)]
pub(crate) struct EchoModelClient;

#[async_trait]
impl ModelClient for EchoModelClient {
    async fn complete(&self, request: &ModelRequest) -> Result<ModelResponse> {
        let text = format!("[Echo fallback] {}", request.user_text());
        Ok(ModelResponse {
            content: vec![AssistantContentBlock::Text { text }],
            stop_reason: Some(StopReason::EndTurn),
            ..ModelResponse::default()
        })
    }

    async fn complete_stream(
        &self,
        request: &ModelRequest,
        delta_sender: Option<UnboundedSender<StreamDelta>>,
    ) -> Result<ModelResponse> {
        let text = format!("[Echo fallback] {}", request.user_text());
        if let Some(sender) = delta_sender {
            let _ = sender.send(StreamDelta::Text(text.clone()));
        }
        Ok(ModelResponse {
            content: vec![AssistantContentBlock::Text { text }],
            stop_reason: Some(StopReason::EndTurn),
            ..ModelResponse::default()
        })
    }

    fn name(&self) -> &'static str {
        "echo"
    }
}
