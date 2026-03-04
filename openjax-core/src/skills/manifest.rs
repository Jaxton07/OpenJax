use serde_json::{Map, Value};

use crate::skills::errors::SkillManifestError;
use crate::skills::types::SkillManifest;

pub fn parse_skill_manifest(
    content: &str,
    fallback_name: &str,
) -> Result<SkillManifest, SkillManifestError> {
    let (frontmatter, body) = split_frontmatter(content)?;
    let mut name = fallback_name.trim().to_string();
    let mut description = String::new();
    let mut extra = Value::Object(Map::new());

    if let Some(raw_frontmatter) = frontmatter.as_deref() {
        let yaml_value: serde_yaml::Value = serde_yaml::from_str(raw_frontmatter)
            .map_err(|err| SkillManifestError::FrontmatterYaml(err.to_string()))?;
        let mut json_value =
            serde_json::to_value(yaml_value).unwrap_or_else(|_| Value::Object(Map::new()));
        if let Value::Object(map) = &mut json_value {
            if let Some(v) = map.remove("name").and_then(value_as_string) {
                name = v;
            }
            if let Some(v) = map.remove("description").and_then(value_as_string) {
                description = v;
            }
            extra = Value::Object(map.clone());
        }
    }

    Ok(SkillManifest {
        name,
        description,
        instructions_markdown: body,
        extra,
    })
}

fn split_frontmatter(content: &str) -> Result<(Option<String>, String), SkillManifestError> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Ok((None, String::new()));
    }

    if trim_cr(lines[0]) != "---" {
        return Ok((None, content.to_string()));
    }

    let mut closing_index = None;
    for (idx, line) in lines.iter().enumerate().skip(1) {
        if trim_cr(line) == "---" {
            closing_index = Some(idx);
            break;
        }
    }

    let closing_index =
        closing_index.ok_or(SkillManifestError::MissingFrontmatterClosingDelimiter)?;
    let frontmatter = lines[1..closing_index].join("\n");
    let body = if closing_index + 1 < lines.len() {
        lines[closing_index + 1..].join("\n")
    } else {
        String::new()
    };
    Ok((Some(frontmatter), body))
}

fn trim_cr(input: &str) -> &str {
    input.strip_suffix('\r').unwrap_or(input)
}

fn value_as_string(value: Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_skill_manifest;

    #[test]
    fn parses_frontmatter_and_body() {
        let content = r#"---
name: Rust Debug Skill
description: Helps debug rust issues
model: claude-sonnet
---
Use `cargo test` first.
"#;
        let manifest = parse_skill_manifest(content, "fallback").expect("manifest parse");
        assert_eq!(manifest.name, "Rust Debug Skill");
        assert_eq!(manifest.description, "Helps debug rust issues");
        assert!(manifest.instructions_markdown.contains("cargo test"));
        assert!(manifest.extra.get("model").is_some());
    }

    #[test]
    fn falls_back_when_frontmatter_missing() {
        let content = "do something helpful";
        let manifest = parse_skill_manifest(content, "fallback-skill").expect("manifest parse");
        assert_eq!(manifest.name, "fallback-skill");
        assert_eq!(manifest.description, "");
        assert_eq!(manifest.instructions_markdown, "do something helpful");
    }

    #[test]
    fn returns_error_for_unclosed_frontmatter() {
        let content = r#"---
name: Broken
description: test
"#;
        let error = parse_skill_manifest(content, "fallback").expect_err("expected parse error");
        assert!(
            error
                .to_string()
                .contains("no closing '---' delimiter was found")
        );
    }

    #[test]
    fn returns_error_for_invalid_frontmatter_yaml() {
        let content = r#"---
name: [
---
body
"#;
        let error = parse_skill_manifest(content, "fallback").expect_err("expected parse error");
        assert!(error.to_string().contains("yaml parse error"));
    }

    #[test]
    fn supports_empty_body() {
        let content = r#"---
name: Empty Body
description: test
---
"#;
        let manifest = parse_skill_manifest(content, "fallback").expect("manifest parse");
        assert!(manifest.instructions_markdown.is_empty());
    }
}
