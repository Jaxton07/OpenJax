#[tokio::main]
async fn main() -> anyhow::Result<()> {
    openjax_gateway::run_stdio().await
}
