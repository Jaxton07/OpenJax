use serde::Deserialize;

/// OpenJax configuration
#[derive(Debug, Deserialize)]
pub struct Config {
    /// Model configuration
    #[serde(default)]
    pub model: Option<ModelConfig>,

    /// Sandbox configuration
    #[serde(default)]
    pub sandbox: Option<SandboxConfig>,

    /// Agent configuration
    #[serde(default)]
    pub agent: Option<AgentConfig>,
}

#[derive(Debug, Deserialize)]
pub struct ModelConfig {
    /// Model backend: minimax | openai | echo
    #[serde(default)]
    pub backend: Option<String>,

    /// API key (optional, can also use env vars)
    #[serde(default)]
    pub api_key: Option<String>,

    /// Base URL override
    #[serde(default)]
    pub base_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SandboxConfig {
    /// Sandbox mode: workspace_write | danger_full_access
    #[serde(default)]
    pub mode: Option<String>,

    /// Approval policy: always_ask | on_request | never
    #[serde(default)]
    pub approval_policy: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AgentConfig {
    /// Maximum concurrent agents
    #[serde(default)]
    pub max_agents: Option<usize>,

    /// Maximum agent depth
    #[serde(default)]
    pub max_depth: Option<i32>,
}

impl Config {
    /// Load config from file
    pub fn from_file(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}
