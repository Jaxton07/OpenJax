use crate::skills::types::SkillMatch;

const INSTRUCTIONS_PREVIEW_CHARS: usize = 420;
const SKILLS_RUNTIME_GUIDANCE: &str = "- Runtime guidance:\n\
  - Skill trigger syntax like `/skill-name` is an invocation marker, not a shell executable.\n\
  - Do NOT call shell with only a trigger string (for example `/xxx`); execute concrete workflow steps instead.\n\
  - For commit workflows, prefer lightweight inspection first: `git status --short` + `git diff --stat`, then expand diff only if needed.\n\
  - Prefer separate `git add` and `git commit` calls over one chained command when possible.\n";

pub fn build_skills_context(selected: &[SkillMatch], max_prompt_chars: usize) -> String {
    if selected.is_empty() {
        return "(none)".to_string();
    }

    let mut output = String::new();
    output.push_str(SKILLS_RUNTIME_GUIDANCE);
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
        truncate_chars(output.trim_end(), max_prompt_chars)
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
                    slash_command: None,
                    instructions_markdown: "x".repeat(600),
                    extra: serde_json::json!({}),
                },
            },
            score: 42,
        };

        let ctx = build_skills_context(&[selection], 120);
        assert!(ctx.chars().count() <= 123);
    }

    #[test]
    fn includes_runtime_guidance_for_selected_skills() {
        let selection = SkillMatch {
            entry: SkillEntry {
                id: "local-commit".to_string(),
                normalized_name: "local-commit".to_string(),
                package_dir: PathBuf::from("/tmp/local-commit"),
                manifest_path: PathBuf::from("/tmp/local-commit/SKILL.md"),
                source_scope: SkillSourceScope::Workspace,
                manifest: SkillManifest {
                    name: "local-commit".to_string(),
                    description: "commit changes".to_string(),
                    slash_command: None,
                    instructions_markdown: "Do commit workflow".to_string(),
                    extra: serde_json::json!({}),
                },
            },
            score: 11,
        };

        let ctx = build_skills_context(&[selection], 6000);
        assert!(ctx.contains("Skill trigger syntax like `/skill-name`"));
        assert!(ctx.contains("Do NOT call shell with only a trigger string"));
        assert!(ctx.contains("git status --short"));
        assert!(ctx.contains("git diff --stat"));
    }
}
