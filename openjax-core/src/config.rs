use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const DEFAULT_CONFIG_TEMPLATE: &str = r#"# OpenJax default config template
# Auto-generated on first startup when no config file is found.
# Fill API keys via environment variables.
#
# Required env vars for this template:
# - OPENJAX_KIMI_API_KEY
# - OPENJAX_GLM_API_KEY
# - OPENAI_API_KEY
# - OPENJAX_ANTHROPIC_API_KEY

[model.routing]
planner = "kimi_default"
final_writer = "kimi_default"
tool_reasoning = "kimi_default"

[model.routing.fallbacks]
kimi_default = ["glm_fast", "openai_default", "claude_default"]
glm_fast = ["openai_default", "claude_default"]
openai_default = ["claude_default"]

[model.models.kimi_default]
provider = "kimi"
protocol = "anthropic_messages"
model = "K2.5"
base_url = "https://api.kimi.com/coding/"
api_key_env = "OPENJAX_KIMI_API_KEY"
thinking_budget_tokens = 4000
supports_stream = true
supports_reasoning = true

[model.models.glm_fast]
provider = "glm"
protocol = "anthropic_messages"
model = "GLM-4.7-FlashX"
base_url = "https://open.bigmodel.cn/api/anthropic"
api_key_env = "OPENJAX_GLM_API_KEY"
thinking_budget_tokens = 2000
supports_stream = true
supports_reasoning = true

[model.models.openai_default]
provider = "openai"
protocol = "chat_completions"
model = "gpt-4.1-mini"
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
supports_stream = true
supports_reasoning = false

[model.models.claude_default]
provider = "anthropic"
protocol = "anthropic_messages"
model = "claude-sonnet-4-5"
base_url = "https://api.anthropic.com/v1"
api_key_env = "OPENJAX_ANTHROPIC_API_KEY"
thinking_budget_tokens = 2000
supports_stream = true
supports_reasoning = true

[sandbox]
mode = "workspace_write"
approval_policy = "on_request"

[agent]
max_agents = 4
max_depth = 1

[skills]
enabled = true
max_selected = 3
max_prompt_chars = 6000
"#;

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

    /// Tools configuration
    #[serde(default)]
    pub tools: Option<crate::tools::spec::ToolsConfig>,

    /// Skills configuration
    #[serde(default)]
    pub skills: Option<SkillsConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ModelConfig {
    /// Legacy model backend: anthropic | glm | minimax | openai | echo
    #[serde(default)]
    pub backend: Option<String>,

    /// Legacy API key (optional, can also use env vars)
    #[serde(default)]
    pub api_key: Option<String>,

    /// Legacy base URL override
    #[serde(default)]
    pub base_url: Option<String>,

    /// Legacy model name
    #[serde(default)]
    pub model: Option<String>,

    /// New named model registry
    #[serde(default)]
    pub models: HashMap<String, ProviderModelConfig>,

    /// New static stage routing config
    #[serde(default)]
    pub routing: Option<ModelRoutingConfig>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ProviderModelConfig {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub protocol: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub anthropic_version: Option<String>,
    #[serde(default)]
    pub thinking_budget_tokens: Option<u32>,
    #[serde(default)]
    pub supports_stream: Option<bool>,
    #[serde(default)]
    pub supports_reasoning: Option<bool>,
    #[serde(default)]
    pub supports_tool_call: Option<bool>,
    #[serde(default)]
    pub supports_json_mode: Option<bool>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ModelRoutingConfig {
    #[serde(default)]
    pub planner: Option<String>,
    #[serde(default)]
    pub final_writer: Option<String>,
    #[serde(default)]
    pub tool_reasoning: Option<String>,
    #[serde(default)]
    pub fallbacks: HashMap<String, Vec<String>>,
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

#[derive(Debug, Deserialize, Clone)]
pub struct SkillsConfig {
    #[serde(default)]
    pub enabled: Option<bool>,

    #[serde(default)]
    pub max_selected: Option<usize>,

    #[serde(default)]
    pub max_prompt_chars: Option<usize>,
}

impl Config {
    /// Load config from file
    pub fn from_file(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Find and load config from default locations
    /// Search order: ./.openjax/config/config.toml -> ~/.openjax/config.toml
    pub fn load() -> Self {
        Self::find_or_create_config_file()
            .and_then(|path| Self::from_file(&path).ok())
            .unwrap_or_default()
    }

    /// Find config file; create default template when not found.
    /// Search/create order: ./.openjax/config/config.toml -> ~/.openjax/config.toml
    pub fn find_or_create_config_file() -> Option<PathBuf> {
        if let Some(existing) = Self::find_config_file() {
            return Some(existing);
        }
        Self::create_default_config_file()
    }

    /// Find config file in default locations
    pub fn find_config_file() -> Option<PathBuf> {
        let cwd_config = cwd_config_path()?;

        if cwd_config.exists() {
            return Some(cwd_config);
        }

        let home_config = home_config_path()?;

        if home_config.exists() {
            return Some(home_config);
        }

        None
    }

    fn create_default_config_file() -> Option<PathBuf> {
        if let Some(cwd_config) = cwd_config_path()
            && write_template_if_missing(&cwd_config).is_ok()
        {
            return Some(cwd_config);
        }

        if let Some(home_config) = home_config_path()
            && write_template_if_missing(&home_config).is_ok()
        {
            return Some(home_config);
        }

        None
    }
}

fn cwd_config_path() -> Option<PathBuf> {
    Some(
        std::env::current_dir()
            .ok()?
            .join(".openjax")
            .join("config")
            .join("config.toml"),
    )
}

fn home_config_path() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".openjax").join("config.toml"))
}

fn write_template_if_missing(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, DEFAULT_CONFIG_TEMPLATE)
}

#[cfg(test)]
mod tests {
    use super::{Config, DEFAULT_CONFIG_TEMPLATE, write_template_if_missing};

    #[test]
    fn write_template_creates_file_and_is_parseable() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let target = tmp
            .path()
            .join(".openjax")
            .join("config")
            .join("config.toml");
        write_template_if_missing(&target).expect("write template");

        let content = std::fs::read_to_string(&target).expect("read config");
        assert!(content.contains("[model.models.kimi_default]"));
        assert!(content.contains("api_key_env = \"OPENJAX_KIMI_API_KEY\""));
        assert!(content.contains("[model.models.claude_default]"));
        assert!(content.contains("[skills]"));
        assert!(content.contains("enabled = true"));
        assert_eq!(content, DEFAULT_CONFIG_TEMPLATE);

        let parsed = Config::from_file(&target).expect("config parse");
        let model = parsed.model.expect("model section");
        assert!(model.models.contains_key("kimi_default"));
        assert!(model.models.contains_key("glm_fast"));
        assert!(model.models.contains_key("openai_default"));
        assert!(model.models.contains_key("claude_default"));
        let skills = parsed.skills.expect("skills section");
        assert_eq!(skills.enabled, Some(true));
        assert_eq!(skills.max_selected, Some(3));
        assert_eq!(skills.max_prompt_chars, Some(6000));
    }

    #[test]
    fn write_template_does_not_overwrite_existing_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let target = tmp
            .path()
            .join(".openjax")
            .join("config")
            .join("config.toml");
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(&target, "custom=true\n").expect("seed existing file");

        write_template_if_missing(&target).expect("write call");

        let content = std::fs::read_to_string(&target).expect("read config");
        assert_eq!(content, "custom=true\n");
    }
}
