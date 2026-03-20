use std::collections::HashSet;

use crate::skills::types::{SkillEntry, SkillMatch, SkillRegistry};

pub fn rank_skills(
    registry: &SkillRegistry,
    user_input: &str,
    max_selected: usize,
) -> Vec<SkillMatch> {
    if max_selected == 0 {
        return Vec::new();
    }

    let input_tokens = tokenize_to_set(user_input);
    if input_tokens.is_empty() {
        return Vec::new();
    }

    let mut scored = Vec::new();
    for entry in &registry.entries {
        let score = score_entry(entry, &input_tokens);
        if score == 0 {
            continue;
        }
        scored.push(SkillMatch {
            entry: entry.clone(),
            score,
        });
    }

    scored.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.entry.manifest.name.cmp(&b.entry.manifest.name))
            .then_with(|| a.entry.id.cmp(&b.entry.id))
    });
    scored.truncate(max_selected);
    scored
}

fn score_entry(entry: &SkillEntry, input_tokens: &HashSet<String>) -> u32 {
    let name_tokens = tokenize_to_set(&entry.manifest.name);
    let id_tokens = tokenize_to_set(&entry.id);
    let description_tokens = tokenize_to_set(&entry.manifest.description);
    let instruction_tokens = tokenize_to_set(&entry.manifest.instructions_markdown);
    let folder_tokens = tokenize_to_set(
        entry
            .package_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default(),
    );

    let mut score = 0u32;
    score += intersection_count(input_tokens, &name_tokens) * 20;
    score += intersection_count(input_tokens, &id_tokens) * 12;
    score += intersection_count(input_tokens, &folder_tokens) * 8;
    score += intersection_count(input_tokens, &description_tokens) * 6;
    score += intersection_count(input_tokens, &instruction_tokens) * 2;
    score
}

fn intersection_count(left: &HashSet<String>, right: &HashSet<String>) -> u32 {
    left.intersection(right).count() as u32
}

fn tokenize_to_set(text: &str) -> HashSet<String> {
    text.split(|ch: char| !ch.is_ascii_alphanumeric())
        .map(str::trim)
        .filter(|token| token.len() >= 2)
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::skills::types::{SkillEntry, SkillManifest, SkillRegistry, SkillSourceScope};

    use super::rank_skills;

    fn entry(name: &str, description: &str, body: &str) -> SkillEntry {
        SkillEntry {
            id: name.to_ascii_lowercase(),
            normalized_name: name.to_ascii_lowercase(),
            package_dir: PathBuf::from(format!("/tmp/{name}")),
            manifest_path: PathBuf::from(format!("/tmp/{name}/SKILL.md")),
            source_scope: SkillSourceScope::Workspace,
            manifest: SkillManifest {
                name: name.to_string(),
                description: description.to_string(),
                slash_command: None,
                instructions_markdown: body.to_string(),
                extra: serde_json::json!({}),
            },
        }
    }

    #[test]
    fn rank_prefers_name_and_description_hits() {
        let registry = SkillRegistry {
            entries: vec![
                entry("rust-debug", "debug rust compile errors", "cargo check"),
                entry("frontend-css", "style ui layout", "tailwind"),
            ],
            loaded_at: std::time::SystemTime::now(),
            discovered_count: 2,
        };

        let ranked = rank_skills(&registry, "help debug rust error", 3);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].entry.manifest.name, "rust-debug");
    }
}
