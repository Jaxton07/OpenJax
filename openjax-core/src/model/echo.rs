use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedSender;

use crate::model::client::ModelClient;

#[derive(Debug, Default)]
pub(crate) struct EchoModelClient;

#[async_trait]
impl ModelClient for EchoModelClient {
    async fn complete(&self, user_input: &str) -> Result<String> {
        Ok(format!("[Echo fallback] {user_input}"))
    }

    async fn complete_stream(
        &self,
        user_input: &str,
        delta_sender: Option<UnboundedSender<String>>,
    ) -> Result<String> {
        let text = format!("[Echo fallback] {user_input}");
        if let Some(sender) = delta_sender {
            let _ = sender.send(text.clone());
        }
        Ok(text)
    }

    fn name(&self) -> &'static str {
        "echo"
    }
}
