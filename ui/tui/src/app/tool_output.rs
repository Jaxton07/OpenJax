pub(crate) fn summarize_tool_output(output: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for raw_line in output.lines() {
        for segment in split_embedded_line_markers(raw_line) {
            let cleaned = strip_leading_line_marker(segment.trim()).trim();
            if cleaned.is_empty() {
                continue;
            }
            lines.push(truncate_chars(cleaned, 96));
        }
    }

    if lines.is_empty() {
        return vec!["(no output)".to_string()];
    }
    if lines.len() <= 4 {
        return lines;
    }

    vec![
        lines[0].clone(),
        format!("… +{} lines", lines.len().saturating_sub(2)),
        lines.last().cloned().unwrap_or_default(),
    ]
}

pub(crate) fn extract_backend_summary(output: &str) -> Option<String> {
    let backend = output
        .lines()
        .find_map(|line| line.strip_prefix("backend=").map(str::trim))?;
    let label = match backend {
        "macos_seatbelt" => "sandbox-exec (macos_seatbelt)".to_string(),
        "linux_native" => "bwrap (linux_native)".to_string(),
        "none_escalated" => "none (degraded)".to_string(),
        other => other.to_string(),
    };
    Some(format!("sandbox: {label}"))
}

pub(crate) fn sanitize_target_for_title(target: &str, max_chars: usize) -> String {
    let collapsed = collapse_whitespace(target);
    truncate_chars(&collapsed, max_chars)
}

pub(crate) fn degraded_risk_summary(output: &str) -> Option<String> {
    let backend = output
        .lines()
        .find_map(|line| line.strip_prefix("backend=").map(str::trim))?;
    if backend != "none_escalated" {
        return None;
    }

    let command = output
        .lines()
        .find_map(|line| line.strip_prefix("command=").map(str::trim))
        .unwrap_or_default()
        .to_ascii_lowercase();
    let policy_decision = output
        .lines()
        .find_map(|line| line.strip_prefix("policy_decision=").map(str::trim))
        .unwrap_or_default()
        .to_ascii_lowercase();

    let mutating = is_mutating_command(&command) || policy_decision.contains("askapproval");
    if mutating {
        Some("risk: mutating command ran unsandboxed".to_string())
    } else {
        Some("degraded: executed outside sandbox".to_string())
    }
}

pub(crate) fn skill_trigger_guard_hint(output: &str) -> Option<String> {
    if output.contains("runtime_deny_reason=skill_trigger_not_shell_command") {
        Some("hint: detected skill trigger string in shell; use skill workflow steps".to_string())
    } else {
        None
    }
}

fn split_embedded_line_markers(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut starts = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'L' && (i == 0 || bytes[i - 1].is_ascii_whitespace()) {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 && j < bytes.len() && bytes[j] == b':' {
                starts.push(i);
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }

    if starts.len() <= 1 {
        return vec![text];
    }

    let mut out = Vec::new();
    for idx in 0..starts.len() {
        let start = starts[idx];
        let end = if idx + 1 < starts.len() {
            starts[idx + 1]
        } else {
            bytes.len()
        };
        out.push(text[start..end].trim());
    }
    out
}

fn strip_leading_line_marker(text: &str) -> &str {
    let bytes = text.as_bytes();
    if bytes.first() != Some(&b'L') {
        return text;
    }
    let mut idx = 1usize;
    while idx < bytes.len() && bytes[idx].is_ascii_digit() {
        idx += 1;
    }
    if idx == 1 || idx >= bytes.len() || bytes[idx] != b':' {
        return text;
    }
    idx += 1;
    while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
        idx += 1;
    }
    &text[idx..]
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out = String::new();
    for ch in text.chars().take(max_chars) {
        out.push(ch);
    }
    out.push_str("...");
    out
}

fn collapse_whitespace(text: &str) -> String {
    let mut out = String::new();
    let mut prev_space = false;
    for ch in text.replace('\r', " ").replace('\n', " ").chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
            continue;
        }
        out.push(ch);
        prev_space = false;
    }
    out.trim().to_string()
}

fn is_mutating_command(command: &str) -> bool {
    let tokens = [
        "git add ",
        "git commit",
        "git merge",
        "git rebase",
        "git cherry-pick",
        "git reset --hard",
        "git clean -fd",
        "rm ",
        "mv ",
        "cp ",
        "chmod ",
        "chown ",
        "touch ",
        "mkdir ",
        "rmdir ",
        "sed -i",
        "perl -i",
        "truncate ",
        ">",
        ">>",
    ];
    tokens.iter().any(|token| command.contains(token))
}

#[cfg(test)]
mod tests {
    use super::{
        degraded_risk_summary, extract_backend_summary, sanitize_target_for_title,
        skill_trigger_guard_hint, summarize_tool_output,
    };

    #[test]
    fn extracts_and_strips_line_markers() {
        let output = "L12: first L13: second\nL14: third";
        let lines = summarize_tool_output(output);
        assert_eq!(lines, vec!["first", "second", "third"]);
    }

    #[test]
    fn truncates_long_lines() {
        let output = format!("L1: {}", "a".repeat(120));
        let lines = summarize_tool_output(&output);
        assert!(lines[0].ends_with("..."));
        assert!(lines[0].len() <= 99);
    }

    #[test]
    fn extracts_backend_summary() {
        let output = "result_class=success\nbackend=macos_seatbelt\nstdout:\nok";
        assert_eq!(
            extract_backend_summary(output).as_deref(),
            Some("sandbox: sandbox-exec (macos_seatbelt)")
        );
    }

    #[test]
    fn sanitize_target_makes_single_line_summary() {
        let target = "git commit -m \"a\"\n\nnext line";
        let sanitized = sanitize_target_for_title(target, 24);
        assert!(!sanitized.contains('\n'));
        assert!(sanitized.contains("git commit"));
    }

    #[test]
    fn degraded_risk_marks_mutating_unsandboxed() {
        let output =
            "command=git add -A\nbackend=none_escalated\npolicy_decision=AskApproval\nstdout:\n";
        assert_eq!(
            degraded_risk_summary(output).as_deref(),
            Some("risk: mutating command ran unsandboxed")
        );
    }

    #[test]
    fn skill_trigger_guard_emits_hint() {
        let output = "result_class=failure\nruntime_deny_reason=skill_trigger_not_shell_command\n";
        assert!(skill_trigger_guard_hint(output).is_some());
    }
}
