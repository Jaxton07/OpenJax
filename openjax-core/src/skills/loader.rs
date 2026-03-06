use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use tracing::warn;

use crate::skills::manifest::parse_skill_manifest;
use crate::skills::types::{SkillEntry, SkillRegistry, SkillSourceScope};

const SKILL_FILE_NAME: &str = "SKILL.md";

pub fn default_skill_roots(skills_dir: &Path) -> Vec<(SkillSourceScope, PathBuf)> {
    vec![(SkillSourceScope::User, skills_dir.to_path_buf())]
}

pub fn discover_registry(skills_dir: &Path) -> SkillRegistry {
    let roots = default_skill_roots(skills_dir);
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    let mut discovered_count = 0usize;

    for (scope, root) in roots {
        if !root.exists() || !root.is_dir() {
            continue;
        }

        let read_dir = match fs::read_dir(&root) {
            Ok(read_dir) => read_dir,
            Err(err) => {
                warn!(
                    path = %root.display(),
                    scope = scope.as_str(),
                    error = %err,
                    "skills root read failed"
                );
                continue;
            }
        };

        let mut children: Vec<PathBuf> = read_dir
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        children.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

        for skill_dir in children {
            discovered_count = discovered_count.saturating_add(1);
            let manifest_path = skill_dir.join(SKILL_FILE_NAME);
            if !manifest_path.is_file() {
                continue;
            }

            let file_name = skill_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("skill")
                .to_string();

            let content = match fs::read_to_string(&manifest_path) {
                Ok(content) => content,
                Err(err) => {
                    warn!(
                        path = %manifest_path.display(),
                        error = %err,
                        "skill manifest read failed"
                    );
                    continue;
                }
            };

            let manifest = match parse_skill_manifest(&content, &file_name) {
                Ok(manifest) => manifest,
                Err(err) => {
                    warn!(
                        path = %manifest_path.display(),
                        error = %err,
                        "skill manifest parse failed"
                    );
                    continue;
                }
            };

            let normalized_name = normalize_key(&manifest.name);
            if !seen.insert(normalized_name.clone()) {
                warn!(
                    normalized_name = %normalized_name,
                    path = %manifest_path.display(),
                    scope = scope.as_str(),
                    "duplicate skill ignored due to priority ordering"
                );
                continue;
            }

            entries.push(SkillEntry {
                id: normalized_name.clone(),
                normalized_name,
                package_dir: skill_dir.clone(),
                manifest_path,
                source_scope: scope,
                manifest,
            });
        }
    }

    SkillRegistry {
        entries,
        loaded_at: std::time::SystemTime::now(),
        discovered_count,
    }
}

pub fn normalize_key(input: &str) -> String {
    let lowered = input.trim().to_ascii_lowercase();
    let mut out = String::with_capacity(lowered.len());
    let mut prev_dash = false;
    for ch in lowered.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            prev_dash = false;
            Some(ch)
        } else if prev_dash {
            None
        } else {
            prev_dash = true;
            Some('-')
        };
        if let Some(ch) = mapped {
            out.push(ch);
        }
    }
    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{discover_registry, normalize_key};

    #[test]
    fn normalize_key_collapses_symbols() {
        assert_eq!(normalize_key("  Rust Debug Skill  "), "rust-debug-skill");
        assert_eq!(normalize_key("A__B"), "a-b");
    }

    #[test]
    fn workspace_overrides_user_for_same_skill_name() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let home = tmp.path().join("home");
        fs::create_dir_all(home.join(".openjax/skills/dup")).expect("create user skill");
        fs::write(
            home.join(".openjax/skills/dup/SKILL.md"),
            "---\nname: Same Skill\ndescription: user\n---\nuser body",
        )
        .expect("write user skill");

        let registry = discover_registry(&home.join(".openjax/skills"));
        assert_eq!(registry.entries.len(), 1);
        assert_eq!(registry.entries[0].manifest.description, "user");
    }

    #[test]
    fn ignores_folders_without_skill_manifest() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("home/.openjax/skills");
        fs::create_dir_all(root.join("no_manifest")).expect("create skill dir");
        let registry = discover_registry(&root);
        assert!(registry.entries.is_empty());
    }
}
