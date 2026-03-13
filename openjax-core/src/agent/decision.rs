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

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct ToolCallSpec {
    #[serde(alias = "id")]
    pub(crate) tool_call_id: Option<String>,
    #[serde(alias = "name")]
    pub(crate) tool_name: Option<String>,
    #[serde(default)]
    pub(crate) arguments: HashMap<String, Value>,
    #[serde(default)]
    pub(crate) depends_on: Vec<String>,
    #[serde(default)]
    pub(crate) concurrency_group: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct NormalizedToolCall {
    pub(crate) tool_call_id: String,
    pub(crate) tool_name: String,
    pub(crate) args: HashMap<String, String>,
    pub(crate) depends_on: Vec<String>,
    pub(crate) concurrency_group: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ModelDecisionV2 {
    #[serde(alias = "type")]
    pub(crate) action: String,
    #[serde(default)]
    pub(crate) message: Option<String>,
    #[serde(default)]
    pub(crate) tool_calls: Vec<ToolCallSpec>,
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
    if let Some(v2) = parse_model_decision_v2(raw) {
        return Some(collapse_v2_to_v1(v2));
    }

    // Case 1: pure JSON or fenced JSON.
    let candidate = extract_json_candidate(raw);
    if let Ok(parsed) = serde_json::from_str::<ModelDecision>(&candidate) {
        return Some(parsed);
    }

    // Case 2: mixed text (e.g. reasoning + trailing JSON object).
    let mixed = extract_json_object_from_mixed_text(raw)?;
    serde_json::from_str::<ModelDecision>(&mixed).ok()
}

pub(crate) fn parse_model_decision_v2(raw: &str) -> Option<ModelDecisionV2> {
    let candidate = extract_json_candidate(raw);
    if let Ok(parsed) = serde_json::from_str::<ModelDecisionV2>(&candidate) {
        return Some(parsed);
    }

    let mixed = extract_json_object_from_mixed_text(raw)?;
    serde_json::from_str::<ModelDecisionV2>(&mixed).ok()
}

pub(crate) fn normalize_tool_calls(tool_calls: &[ToolCallSpec]) -> Vec<NormalizedToolCall> {
    let mut normalized = Vec::new();
    for (index, item) in tool_calls.iter().enumerate() {
        let Some(tool_name) = item.tool_name.as_ref().map(|name| name.trim()).filter(|name| !name.is_empty()) else {
            continue;
        };
        let tool_name = tool_name.to_ascii_lowercase();
        if !is_supported_tool_name(&tool_name) {
            continue;
        }
        let tool_call_id = item
            .tool_call_id
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("tool_batch_call_{}", index + 1));
        normalized.push(NormalizedToolCall {
            tool_call_id,
            tool_name,
            args: stringify_map_values(&item.arguments),
            depends_on: item.depends_on.clone(),
            concurrency_group: item.concurrency_group.clone(),
        });
    }
    normalized
}

fn is_supported_tool_name(name: &str) -> bool {
    matches!(
        name,
        "read_file"
            | "list_dir"
            | "grep_files"
            | "process_snapshot"
            | "system_load"
            | "disk_usage"
            | "shell"
            | "apply_patch"
            | "edit_file_range"
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

fn stringify_map_values(values: &HashMap<String, Value>) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for (key, value) in values {
        if let Some(parsed) = stringify_json_value(value) {
            result.insert(key.clone(), parsed);
        }
    }
    result
}

fn collapse_v2_to_v1(mut decision: ModelDecisionV2) -> ModelDecision {
    let action_lower = decision.action.to_ascii_lowercase();
    if action_lower == "tool_batch" && !decision.tool_calls.is_empty() {
        let first = decision.tool_calls.remove(0);
        let mut extra = decision.extra;
        if let Some(tool_call_id) = first.tool_call_id.clone() {
            extra.insert("tool_call_id".to_string(), Value::String(tool_call_id));
        }
        if !first.depends_on.is_empty() {
            extra.insert("depends_on".to_string(), Value::from(first.depends_on.clone()));
        }
        if let Some(concurrency_group) = first.concurrency_group.clone() {
            extra.insert(
                "concurrency_group".to_string(),
                Value::String(concurrency_group),
            );
        }
        return ModelDecision {
            action: "tool".to_string(),
            tool: first.tool_name.map(|name| name.to_ascii_lowercase()),
            args: Some(stringify_map_values(&first.arguments)),
            message: decision.message,
            extra,
        };
    }

    let tool_name = decision
        .extra
        .get("tool")
        .and_then(Value::as_str)
        .map(|value| value.to_string());

    let args = decision
        .extra
        .get("args")
        .and_then(|value| value.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(key, value)| stringify_json_value(value).map(|parsed| (key.clone(), parsed)))
                .collect::<HashMap<_, _>>()
        });

    ModelDecision {
        action: decision.action,
        tool: tool_name,
        args,
        message: decision.message,
        extra: decision.extra,
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

    if decision.tool.as_deref().is_none_or(|t| t.trim().is_empty()) {
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

#[cfg(test)]
mod tests {
    use super::{parse_model_decision, parse_model_decision_v2};

    #[test]
    fn parses_v2_tool_batch_shape() {
        let raw = r#"{
          "action":"tool_batch",
          "tool_calls":[
            {"tool_call_id":"call_1","tool_name":"read_file","arguments":{"file_path":"/tmp/a"}}
          ]
        }"#;
        let decision = parse_model_decision_v2(raw).expect("parse v2");
        assert_eq!(decision.action, "tool_batch");
        assert_eq!(decision.tool_calls.len(), 1);
        assert_eq!(decision.tool_calls[0].tool_name.as_deref(), Some("read_file"));
    }

    #[test]
    fn collapses_v2_tool_batch_to_legacy_tool_action() {
        let raw = r#"{
          "action":"tool_batch",
          "tool_calls":[
            {"tool_call_id":"call_1","tool_name":"read_file","arguments":{"file_path":"/tmp/a"}}
          ]
        }"#;
        let decision = parse_model_decision(raw).expect("parse decision");
        assert_eq!(decision.action, "tool");
        assert_eq!(decision.tool.as_deref(), Some("read_file"));
        assert_eq!(
            decision.args.as_ref().and_then(|args| args.get("file_path")),
            Some(&"/tmp/a".to_string())
        );
    }
}
