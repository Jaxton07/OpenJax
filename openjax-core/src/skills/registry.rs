use tracing::info;

use crate::OpenJaxPaths;
use crate::skills::loader::discover_registry;
use crate::skills::matcher::rank_skills;
use crate::skills::types::{SkillMatch, SkillRegistry};

impl SkillRegistry {
    pub fn load_from_default_locations() -> Self {
        let registry = OpenJaxPaths::detect()
            .and_then(|paths| {
                paths.ensure_runtime_dirs().ok()?;
                Some(discover_registry(&paths.skills_dir))
            })
            .unwrap_or_else(Self::empty);
        info!(
            skills_discovered = registry.discovered_count,
            skills_loaded = registry.entries.len(),
            "skills registry loaded"
        );
        registry
    }

    pub fn load_from_locations(skills_dir: &std::path::Path) -> Self {
        discover_registry(skills_dir)
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
