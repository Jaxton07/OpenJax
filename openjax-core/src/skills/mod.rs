mod errors;
mod loader;
mod manifest;
mod matcher;
mod prompt;
mod registry;
mod types;

pub use errors::SkillManifestError;
pub use loader::{default_skill_roots, discover_registry, normalize_key};
pub use manifest::parse_skill_manifest;
pub use prompt::build_skills_context;
pub use types::{
    DEFAULT_SKILLS_ENABLED, DEFAULT_SKILLS_MAX_PROMPT_CHARS, DEFAULT_SKILLS_MAX_SELECTED,
    SkillEntry, SkillManifest, SkillMatch, SkillRegistry, SkillRuntimeConfig, SkillSourceScope,
};
