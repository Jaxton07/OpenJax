use std::path::PathBuf;
use std::time::SystemTime;

use serde_json::Value;

pub const DEFAULT_SKILLS_ENABLED: bool = true;
pub const DEFAULT_SKILLS_MAX_SELECTED: usize = 3;
pub const DEFAULT_SKILLS_MAX_PROMPT_CHARS: usize = 6_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillSourceScope {
    Workspace,
    User,
}

impl SkillSourceScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::User => "user",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SkillManifest {
    pub name: String,
    pub description: String,
    pub instructions_markdown: String,
    pub extra: Value,
}

#[derive(Debug, Clone)]
pub struct SkillEntry {
    pub id: String,
    pub normalized_name: String,
    pub package_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub source_scope: SkillSourceScope,
    pub manifest: SkillManifest,
}

#[derive(Debug, Clone)]
pub struct SkillMatch {
    pub entry: SkillEntry,
    pub score: u32,
}

#[derive(Debug, Clone)]
pub struct SkillRegistry {
    pub entries: Vec<SkillEntry>,
    pub loaded_at: SystemTime,
    pub discovered_count: usize,
}

impl SkillRegistry {
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
            loaded_at: SystemTime::now(),
            discovered_count: 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SkillRuntimeConfig {
    pub enabled: bool,
    pub max_selected: usize,
    pub max_prompt_chars: usize,
}

impl Default for SkillRuntimeConfig {
    fn default() -> Self {
        Self {
            enabled: DEFAULT_SKILLS_ENABLED,
            max_selected: DEFAULT_SKILLS_MAX_SELECTED,
            max_prompt_chars: DEFAULT_SKILLS_MAX_PROMPT_CHARS,
        }
    }
}

impl SkillRuntimeConfig {
    pub fn from_options(
        enabled: Option<bool>,
        max_selected: Option<usize>,
        max_prompt_chars: Option<usize>,
    ) -> Self {
        let mut config = Self::default();
        if let Some(value) = enabled {
            config.enabled = value;
        }
        if let Some(value) = max_selected {
            config.max_selected = value.max(1);
        }
        if let Some(value) = max_prompt_chars {
            config.max_prompt_chars = value.max(256);
        }
        config
    }

    pub fn apply_env(self) -> Self {
        self.apply_env_with_lookup(|key| std::env::var(key).ok())
    }

    pub fn apply_env_with_lookup<F>(self, lookup: F) -> Self
    where
        F: Fn(&str) -> Option<String>,
    {
        let mut updated = self;

        if let Some(raw) = lookup("OPENJAX_SKILLS_ENABLED")
            && let Some(parsed) = parse_bool(&raw)
        {
            updated.enabled = parsed;
        }

        if let Some(raw) = lookup("OPENJAX_SKILLS_MAX_SELECTED")
            && let Ok(parsed) = raw.trim().parse::<usize>()
        {
            updated.max_selected = parsed.max(1);
        }

        if let Some(raw) = lookup("OPENJAX_SKILLS_MAX_PROMPT_CHARS")
            && let Ok(parsed) = raw.trim().parse::<usize>()
        {
            updated.max_prompt_chars = parsed.max(256);
        }

        updated
    }
}

fn parse_bool(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}
