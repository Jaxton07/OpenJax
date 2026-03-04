use crate::skills::types::SkillMatch;

const INSTRUCTIONS_PREVIEW_CHARS: usize = 420;

pub fn build_skills_context(selected: &[SkillMatch], max_prompt_chars: usize) -> String {
    if selected.is_empty() {
        return "(none)".to_string();
    }

    let mut output = String::new();
    for skill in selected {
        let block = render_skill_block(skill);
        if output.chars().count() + block.chars().count() > max_prompt_chars {
            break;
        }
        output.push_str(&block);
    }

    if output.is_empty() {
        truncate_chars(&render_skill_block(&selected[0]), max_prompt_chars)
    } else {
        output.trim_end().to_string()
    }
}

fn render_skill_block(skill: &SkillMatch) -> String {
    let description = if skill.entry.manifest.description.trim().is_empty() {
        "(no description)".to_string()
    } else {
        skill.entry.manifest.description.replace('\n', " ")
    };
    let instructions = truncate_chars(
        &skill.entry.manifest.instructions_markdown,
        INSTRUCTIONS_PREVIEW_CHARS,
    )
    .replace('\n', "\\n");

    format!(
        "- name: {}\n  description: {}\n  path: {}\n  score: {}\n  instructions: {}\n",
        skill.entry.manifest.name,
        description,
        skill.entry.package_dir.display(),
        skill.score,
        instructions
    )
}

fn truncate_chars(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut output = text.chars().take(limit).collect::<String>();
    output.push_str("...");
    output
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::skills::types::{SkillEntry, SkillManifest, SkillMatch, SkillSourceScope};

    use super::build_skills_context;

    #[test]
    fn truncates_when_prompt_budget_is_small() {
        let selection = SkillMatch {
            entry: SkillEntry {
                id: "rust-debug".to_string(),
                normalized_name: "rust-debug".to_string(),
                package_dir: PathBuf::from("/tmp/rust-debug"),
                manifest_path: PathBuf::from("/tmp/rust-debug/SKILL.md"),
                source_scope: SkillSourceScope::Workspace,
                manifest: SkillManifest {
                    name: "rust-debug".to_string(),
                    description: "debug rust".to_string(),
                    instructions_markdown: "x".repeat(600),
                    extra: serde_json::json!({}),
                },
            },
            score: 42,
        };

        let ctx = build_skills_context(&[selection], 120);
        assert!(ctx.chars().count() <= 123);
    }
}
