pub fn normalize_patch_arg(raw: &str) -> String {
    if raw.contains('\n') {
        raw.to_string()
    } else if raw.contains("\\n") {
        raw.replace("\\n", "\n")
    } else if let Some(content) = extract_heredoc(raw) {
        content
    } else {
        raw.to_string()
    }
}

pub fn extract_heredoc(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if !trimmed.starts_with("<<") {
        return None;
    }

    let rest = &trimmed[2..];
    let (delimiter, content_start) = if let Some(stripped) = rest.strip_prefix('\'') {
        let end_quote = stripped.find('\'')?;
        let delimiter = &stripped[..end_quote];
        let content_start = &stripped[end_quote + 1..];
        (delimiter, content_start)
    } else if let Some(stripped) = rest.strip_prefix('"') {
        let end_quote = stripped.find('"')?;
        let delimiter = &stripped[..end_quote];
        let content_start = &stripped[end_quote + 1..];
        (delimiter, content_start)
    } else {
        let space_pos = rest.find(|c: char| c.is_whitespace())?;
        let delimiter = &rest[..space_pos];
        let content_start = &rest[space_pos..];
        (delimiter, content_start)
    };

    let content_start = content_start.trim_start_matches(|c: char| c.is_whitespace() && c != '\n');
    if !content_start.starts_with('\n') {
        return None;
    }

    let content = &content_start[1..];
    let end_marker = format!("\n{}", delimiter);
    let end_pos = content.find(&end_marker)?;

    Some(content[..end_pos].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_heredoc_single_quote() {
        let raw = "<<'EOF'\n*** Begin Patch\n*** Add File: test.txt\n+content\n*** End Patch\nEOF";
        let result = extract_heredoc(raw);
        assert!(result.is_some());
        assert_eq!(
            result.unwrap(),
            "*** Begin Patch\n*** Add File: test.txt\n+content\n*** End Patch"
        );
    }

    #[test]
    fn extract_heredoc_double_quote() {
        let raw =
            "<<\"EOF\"\n*** Begin Patch\n*** Add File: test.txt\n+content\n*** End Patch\nEOF";
        let result = extract_heredoc(raw);
        assert!(result.is_some());
        assert_eq!(
            result.unwrap(),
            "*** Begin Patch\n*** Add File: test.txt\n+content\n*** End Patch"
        );
    }

    #[test]
    fn extract_heredoc_unquoted() {
        let raw = "<<EOF\n*** Begin Patch\n*** Add File: test.txt\n+content\n*** End Patch\nEOF";
        let result = extract_heredoc(raw);
        assert!(result.is_some());
        assert_eq!(
            result.unwrap(),
            "*** Begin Patch\n*** Add File: test.txt\n+content\n*** End Patch"
        );
    }

    #[test]
    fn extract_heredoc_no_match() {
        let raw = "not a heredoc";
        let result = extract_heredoc(raw);
        assert!(result.is_none());
    }
}
