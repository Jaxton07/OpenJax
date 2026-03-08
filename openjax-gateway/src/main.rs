use anyhow::Result;
use openjax_core::init_logger_with_file;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    init_logger_with_file("openjax-gateway.log");
    let state = openjax_gateway::AppState::new();
    let app = openjax_gateway::build_app(state);
    let bind_addr =
        std::env::var("OPENJAX_GATEWAY_BIND").unwrap_or_else(|_| "127.0.0.1:8765".to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    info!(bind_addr = %bind_addr, "openjax-gateway listening");
    axum::serve(listener, app).await?;
    Ok(())
}
