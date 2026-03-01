#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tui_next::run().await
}
