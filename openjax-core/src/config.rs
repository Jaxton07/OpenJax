use serde::Deserialize;
use std::path::PathBuf;

/// OpenJax configuration
#[derive(Debug, Deserialize, Default)]
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

#[derive(Debug, Deserialize, Clone)]
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

    /// Model name
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SandboxConfig {
    /// Sandbox mode: workspace_write | danger_full_access
    #[serde(default)]
    pub mode: Option<String>,

    /// Approval policy: always_ask | on_request | never
    #[serde(default)]
    pub approval_policy: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
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

    /// Find and load config from default locations
    /// Search order: ./.openjax.toml -> ~/.openjax/config.toml
    pub fn load() -> Self {
        Self::find_config_file()
            .and_then(|path| Self::from_file(&path).ok())
            .unwrap_or_default()
    }

    /// Find config file in default locations
    pub fn find_config_file() -> Option<PathBuf> {
        let cwd_config = std::env::current_dir()
            .ok()?
            .join(".openjax.toml");
        
        if cwd_config.exists() {
            return Some(cwd_config);
        }

        let home_config = dirs::home_dir()?
            .join(".openjax")
            .join("config.toml");
        
        if home_config.exists() {
            return Some(home_config);
        }

        None
    }
}
