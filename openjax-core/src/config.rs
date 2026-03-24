use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::OpenJaxPaths;

const DEFAULT_CONFIG_TEMPLATE: &str = r#"# OpenJax default config template
# Auto-generated on first startup when no config file is found.
#
# LLM provider / API key configuration is managed via the WebUI
# (Settings → Providers). Changes there are persisted to the database
# and take effect immediately — no edits to this file are needed.

[sandbox]
mode = "workspace_write"

[agent]
max_agents = 4
max_depth = 1
max_tool_calls_per_turn = 10
max_planner_rounds_per_turn = 20

[skills]
enabled = true
max_selected = 3
max_prompt_chars = 6000
prevent_shell_skill_trigger = true
prefer_lightweight_git_inspection = true
max_diff_chars_for_planner = 4000
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
    pub request_profile: Option<String>,
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
    #[serde(default)]
    pub context_window_size: Option<u32>,
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
}

#[derive(Debug, Deserialize, Clone)]
pub struct AgentConfig {
    /// Maximum concurrent agents
    #[serde(default)]
    pub max_agents: Option<usize>,

    /// Maximum agent depth
    #[serde(default)]
    pub max_depth: Option<i32>,

    /// Maximum tool calls allowed in one user turn
    #[serde(default)]
    pub max_tool_calls_per_turn: Option<usize>,

    /// Maximum planner rounds allowed in one user turn
    #[serde(default)]
    pub max_planner_rounds_per_turn: Option<usize>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SkillsConfig {
    #[serde(default)]
    pub enabled: Option<bool>,

    #[serde(default)]
    pub max_selected: Option<usize>,

    #[serde(default)]
    pub max_prompt_chars: Option<usize>,

    #[serde(default)]
    pub prevent_shell_skill_trigger: Option<bool>,

    #[serde(default)]
    pub prefer_lightweight_git_inspection: Option<bool>,

    #[serde(default)]
    pub max_diff_chars_for_planner: Option<usize>,
}

impl Config {
    /// Load config from file
    pub fn from_file(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Find and load config from the OpenJax user directory.
    pub fn load() -> Self {
        OpenJaxPaths::detect()
            .and_then(|paths| Self::find_or_create_config_file_at(&paths))
            .and_then(|path| Self::from_file(&path).ok())
            .unwrap_or_default()
    }

    /// Find config file in the OpenJax user directory; create default template when missing.
    pub fn find_or_create_config_file() -> Option<PathBuf> {
        OpenJaxPaths::detect().and_then(|paths| Self::find_or_create_config_file_at(&paths))
    }

    /// Find config file in the OpenJax user directory.
    pub fn find_config_file() -> Option<PathBuf> {
        OpenJaxPaths::detect().and_then(|paths| Self::find_config_file_at(&paths))
    }

    fn find_or_create_config_file_at(paths: &OpenJaxPaths) -> Option<PathBuf> {
        if let Some(existing) = Self::find_config_file_at(paths) {
            return Some(existing);
        }
        Self::create_default_config_file_at(paths)
    }

    fn find_config_file_at(paths: &OpenJaxPaths) -> Option<PathBuf> {
        let config_file = paths.config_file.clone();
        config_file.exists().then_some(config_file)
    }

    fn create_default_config_file_at(paths: &OpenJaxPaths) -> Option<PathBuf> {
        paths.ensure_runtime_dirs().ok()?;
        write_template_if_missing(&paths.config_file).ok()?;
        Some(paths.config_file.clone())
    }
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
    use crate::OpenJaxPaths;

    #[test]
    fn write_template_creates_file_and_is_parseable() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let target = tmp.path().join(".openjax").join("config.toml");
        write_template_if_missing(&target).expect("write template");

        let content = std::fs::read_to_string(&target).expect("read config");
        // Model/provider config is no longer stored in config.toml.
        assert!(!content.contains("[model.models"));
        assert!(!content.contains("api_key_env"));
        assert!(content.contains("max_tool_calls_per_turn = 10"));
        assert!(content.contains("max_planner_rounds_per_turn = 20"));
        assert!(content.contains("[skills]"));
        assert!(content.contains("enabled = true"));
        assert_eq!(content, DEFAULT_CONFIG_TEMPLATE);

        let parsed = Config::from_file(&target).expect("config parse");
        // No model section in the default template.
        assert!(parsed.model.is_none());
        let skills = parsed.skills.expect("skills section");
        assert_eq!(skills.enabled, Some(true));
        assert_eq!(skills.max_selected, Some(3));
        assert_eq!(skills.max_prompt_chars, Some(6000));
        assert_eq!(skills.prevent_shell_skill_trigger, Some(true));
        assert_eq!(skills.prefer_lightweight_git_inspection, Some(true));
        assert_eq!(skills.max_diff_chars_for_planner, Some(4000));
        let agent = parsed.agent.expect("agent section");
        assert_eq!(agent.max_tool_calls_per_turn, Some(10));
        assert_eq!(agent.max_planner_rounds_per_turn, Some(20));
    }

    #[test]
    fn write_template_does_not_overwrite_existing_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let target = tmp.path().join(".openjax").join("config.toml");
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(&target, "custom=true\n").expect("seed existing file");

        write_template_if_missing(&target).expect("write call");

        let content = std::fs::read_to_string(&target).expect("read config");
        assert_eq!(content, "custom=true\n");
    }

    #[test]
    fn find_or_create_uses_only_user_root_config_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = OpenJaxPaths::from_home_dir(tmp.path());

        let created = Config::find_or_create_config_file_at(&paths).expect("config path");

        assert_eq!(created, paths.config_file);
        assert!(paths.config_file.is_file());
        assert!(paths.logs_dir.is_dir());
        assert!(paths.skills_dir.is_dir());
        assert!(!tmp.path().join("workspace/.openjax/config.toml").exists());
    }

    #[test]
    fn parses_model_request_profile_from_toml() {
        let raw = r#"
[model.models.kimi]
provider = "kimi"
protocol = "chat_completions"
model = "kimi-for-coding"
request_profile = "kimi_coding_v1"
"#;

        let config: Config = toml::from_str(raw).expect("parse config");
        let profile = config
            .model
            .as_ref()
            .and_then(|model| model.models.get("kimi"))
            .and_then(|entry| entry.request_profile.as_deref());
        assert_eq!(profile, Some("kimi_coding_v1"));
    }
}
