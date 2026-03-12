use anyhow::Result;
use openjax_core::init_logger_with_file;
use openjax_gateway::{ApiKeySource, AppState, load_api_keys};
use std::path::PathBuf;
use tracing::info;

fn resolve_static_dir() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("OPENJAX_GATEWAY_WEB_DIR") {
        let candidate = PathBuf::from(path);
        if candidate.join("index.html").is_file() {
            return Some(candidate);
        }
    }

    let exe = std::env::current_exe().ok()?;
    let bin_dir = exe.parent()?;
    let candidate = bin_dir.join("../web");
    if candidate.join("index.html").is_file() {
        return Some(candidate);
    }
    None
}

#[tokio::main]
async fn main() -> Result<()> {
    init_logger_with_file("openjax-gateway.log");
    let api_key_config = load_api_keys();
    let state = AppState::new_with_api_keys(api_key_config.keys.clone());
    let static_dir = resolve_static_dir();
    let app = openjax_gateway::build_app(state, static_dir.clone());
    let bind_addr =
        std::env::var("OPENJAX_GATEWAY_BIND").unwrap_or_else(|_| "127.0.0.1:8765".to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    info!(
        bind_addr = %bind_addr,
        static_dir = static_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<none>".to_string()),
        key_source = ?api_key_config.source,
        "openjax-gateway listening"
    );
    println!("[openjax-gateway] listening on http://{bind_addr}");
    if api_key_config.source == ApiKeySource::Generated {
        if let Some(generated_key) = api_key_config.generated_key.as_deref() {
            println!("[openjax-gateway] generated access key: {generated_key}");
            println!("[openjax-gateway] copy this key into the Web login page to continue");
        }
    }
    axum::serve(listener, app).await?;
    Ok(())
}
