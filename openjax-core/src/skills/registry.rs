use std::path::Path;

use tracing::info;

use crate::skills::loader::discover_registry;
use crate::skills::matcher::rank_skills;
use crate::skills::types::{SkillMatch, SkillRegistry};

impl SkillRegistry {
    pub fn load_from_default_locations(cwd: &Path) -> Self {
        let home = dirs::home_dir();
        let registry = discover_registry(cwd, home.as_deref());
        info!(
            skills_discovered = registry.discovered_count,
            skills_loaded = registry.entries.len(),
            "skills registry loaded"
        );
        registry
    }

    pub fn load_from_locations(cwd: &Path, home_dir: Option<&Path>) -> Self {
        discover_registry(cwd, home_dir)
    }

    pub fn select_for_input(&self, user_input: &str, max_selected: usize) -> Vec<SkillMatch> {
        rank_skills(self, user_input, max_selected)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
