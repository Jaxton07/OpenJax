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

#[cfg(test)]
mod tests {
    use super::summarize_tool_output;

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
}
