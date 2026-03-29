use std::collections::HashMap;

pub(super) fn extract_tool_target_hint(
    tool_name: &str,
    args: &HashMap<String, String>,
) -> Option<String> {
    let keys: &[&str] = match tool_name {
        "Read" | "Edit" | "write_file" => &["file_path", "path", "filepath"],
        "disk_usage" => &["path"],
        "shell" | "exec_command" => &["cmd", "command"],
        _ => return None,
    };
    keys.iter().find_map(|k| args.get(*k).cloned())
}

pub(super) fn tool_args_delta_payload(args: &HashMap<String, String>) -> Option<String> {
    if args.is_empty() {
        return None;
    }
    serde_json::to_string(args).ok()
}

pub(super) fn tool_failure_code(error_text: &str) -> &'static str {
    let lower = error_text.to_ascii_lowercase();
    if lower.contains("approval timed out") {
        "approval_timeout"
    } else if lower.contains("approval rejected") {
        "approval_rejected"
    } else if lower.contains("timed out") {
        "tool_timeout"
    } else if lower.contains("cancel") {
        "tool_canceled"
    } else {
        "tool_execution_failed"
    }
}

pub(super) fn tool_failure_retryable(error_text: &str) -> bool {
    let lower = error_text.to_ascii_lowercase();
    lower.contains("timed out") || lower.contains("cancel")
}

pub(super) fn is_mutating_tool(tool_name: &str) -> bool {
    matches!(tool_name, "Edit" | "shell" | "exec_command")
}

pub(super) fn summarize_log_preview(text: &str, limit: usize) -> (String, bool) {
    let normalized = text.replace('\n', "\\n").replace('\r', "\\r");
    let total = normalized.chars().count();
    if total <= limit {
        return (normalized, false);
    }

    let mut preview = normalized.chars().take(limit).collect::<String>();
    preview.push_str("...");
    (preview, true)
}

#[allow(dead_code)]
pub(super) fn summarize_log_preview_json(text: &str, limit: usize) -> String {
    let (preview, truncated) = summarize_log_preview(text, limit);
    serde_json::json!({
        "model_output": preview,
        "truncated": truncated,
    })
    .to_string()
}

pub(super) fn looks_like_skill_trigger_shell_command(command: &str) -> bool {
    let trimmed = command.trim();
    if trimmed.is_empty() || !trimmed.starts_with('/') {
        return false;
    }
    if trimmed.contains(char::is_whitespace) {
        return false;
    }
    trimmed[1..].chars().all(|ch| ch != '/')
}

pub(super) fn is_git_status_short(command: &str) -> bool {
    let normalized = command.trim().to_ascii_lowercase();
    normalized == "git status --short" || normalized == "git status -s"
}

pub(super) fn is_git_diff_stat(command: &str) -> bool {
    command
        .trim()
        .to_ascii_lowercase()
        .contains("git diff --stat")
}

pub(super) fn detect_diff_strategy(command: &str) -> Option<&'static str> {
    let normalized = command.trim().to_ascii_lowercase();
    if !normalized.contains("git diff") {
        return None;
    }
    if normalized.contains("git diff --stat") {
        return Some("stat_only");
    }
    if normalized.contains("git diff --staged")
        || normalized.contains("git diff --cached")
        || normalized.contains("git diff -- ")
    {
        return Some("targeted");
    }
    Some("full")
}

pub(super) fn merge_diff_strategy(current: &str, next: &str) -> &'static str {
    fn rank(value: &str) -> u8 {
        match value {
            "stat_only" => 1,
            "targeted" => 2,
            "full" => 3,
            _ => 0,
        }
    }
    if rank(next) >= rank(current) {
        match next {
            "stat_only" => "stat_only",
            "targeted" => "targeted",
            "full" => "full",
            _ => "none",
        }
    } else {
        match current {
            "stat_only" => "stat_only",
            "targeted" => "targeted",
            "full" => "full",
            _ => "none",
        }
    }
}
