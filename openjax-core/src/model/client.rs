use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedSender;

#[async_trait]
pub trait ModelClient: Send + Sync {
    async fn complete(&self, user_input: &str) -> Result<String>;

    async fn complete_stream(
        &self,
        user_input: &str,
        delta_sender: Option<UnboundedSender<String>>,
    ) -> Result<String>;

    fn name(&self) -> &'static str;
}
