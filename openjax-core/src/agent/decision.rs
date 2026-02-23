use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub(crate) struct ModelDecision {
    #[serde(alias = "type")]
    pub(crate) action: String,
    pub(crate) tool: Option<String>,
    pub(crate) args: Option<HashMap<String, String>>,
    pub(crate) message: Option<String>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, Value>,
}

fn extract_json_candidate(raw: &str) -> String {
    let trimmed = raw.trim();

    if !trimmed.starts_with("```") {
        return trimmed.to_string();
    }

    let mut lines = trimmed.lines().collect::<Vec<_>>();
    if lines.first().is_some_and(|line| line.starts_with("```")) {
        lines.remove(0);
    }
    if lines.last().is_some_and(|line| line.trim() == "```") {
        lines.pop();
    }

    lines.join("\n")
}

fn extract_json_object_from_mixed_text(raw: &str) -> Option<String> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(raw[start..=end].to_string())
}

pub(crate) fn parse_model_decision(raw: &str) -> Option<ModelDecision> {
    // Case 1: pure JSON or fenced JSON.
    let candidate = extract_json_candidate(raw);
    if let Ok(parsed) = serde_json::from_str::<ModelDecision>(&candidate) {
        return Some(parsed);
    }

    // Case 2: mixed text (e.g. reasoning + trailing JSON object).
    let mixed = extract_json_object_from_mixed_text(raw)?;
    serde_json::from_str::<ModelDecision>(&mixed).ok()
}

fn is_supported_tool_name(name: &str) -> bool {
    matches!(
        name,
        "read_file" | "list_dir" | "grep_files" | "shell" | "apply_patch" | "edit_file_range"
    )
}

fn stringify_json_value(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => None,
        Value::Array(_) | Value::Object(_) => Some(value.to_string()),
    }
}

pub(crate) fn normalize_model_decision(mut decision: ModelDecision) -> ModelDecision {
    let action_lower = decision.action.to_ascii_lowercase();
    if action_lower == "tool" || action_lower == "final" {
        return decision;
    }

    if !is_supported_tool_name(&action_lower) {
        return decision;
    }

    if decision
        .tool
        .as_deref()
        .map_or(true, |t| t.trim().is_empty())
    {
        decision.tool = Some(action_lower.clone());
    }

    if decision.args.is_none() {
        let mut args = HashMap::new();
        for (k, v) in &decision.extra {
            if matches!(k.as_str(), "action" | "type" | "tool" | "args" | "message") {
                continue;
            }
            if let Some(value) = stringify_json_value(v) {
                args.insert(k.clone(), value);
            }
        }
        if !args.is_empty() {
            decision.args = Some(args);
        }
    }

    decision.action = "tool".to_string();
    decision
}
