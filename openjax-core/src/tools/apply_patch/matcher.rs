use std::path::Path;

pub fn split_lines_preserve_end(content: &str) -> Vec<String> {
    if content.is_empty() {
        return Vec::new();
    }
    content.split('\n').map(|line| line.to_string()).collect()
}

pub fn find_subsequence(haystack: &[String], start: usize, needle: &[String]) -> Option<usize> {
    if needle.is_empty() {
        return Some(start);
    }
    if haystack.len() < needle.len() || start > haystack.len().saturating_sub(needle.len()) {
        return None;
    }
    (start..=haystack.len() - needle.len()).find(|idx| {
        haystack[*idx..*idx + needle.len()]
            .iter()
            .zip(needle)
            .all(|(left, right)| left == right)
    })
}

pub fn seek_sequence(
    lines: &[String],
    pattern: &[String],
    start: usize,
    eof: bool,
) -> Option<usize> {
    if pattern.is_empty() {
        return Some(start);
    }

    let max_start = if eof {
        lines.len()
    } else {
        lines.len().saturating_sub(pattern.len())
    };

    for level in 0..4 {
        match level {
            0 => {
                if let Some(pos) = find_subsequence_exact(lines, start, pattern, max_start) {
                    return Some(pos);
                }
            }
            1 => {
                if let Some(pos) = find_subsequence_trim_end(lines, start, pattern, max_start) {
                    return Some(pos);
                }
            }
            2 => {
                if let Some(pos) = find_subsequence_trim(lines, start, pattern, max_start) {
                    return Some(pos);
                }
            }
            3 => {
                if let Some(pos) = find_subsequence_normalized(lines, start, pattern, max_start) {
                    return Some(pos);
                }
            }
            _ => break,
        }
    }

    None
}

fn find_subsequence_exact(
    lines: &[String],
    start: usize,
    pattern: &[String],
    max_start: usize,
) -> Option<usize> {
    let end = max_start.min(lines.len());
    for i in start..end {
        if i + pattern.len() > lines.len() {
            break;
        }
        if lines[i..i + pattern.len()]
            .iter()
            .zip(pattern)
            .all(|(l, p)| l == p)
        {
            return Some(i);
        }
    }
    None
}

fn find_subsequence_trim_end(
    lines: &[String],
    start: usize,
    pattern: &[String],
    max_start: usize,
) -> Option<usize> {
    let end = max_start.min(lines.len());
    for i in start..end {
        if i + pattern.len() > lines.len() {
            break;
        }
        if lines[i..i + pattern.len()]
            .iter()
            .zip(pattern)
            .all(|(l, p)| l.trim_end() == p.trim_end())
        {
            return Some(i);
        }
    }
    None
}

fn find_subsequence_trim(
    lines: &[String],
    start: usize,
    pattern: &[String],
    max_start: usize,
) -> Option<usize> {
    let end = max_start.min(lines.len());
    for i in start..end {
        if i + pattern.len() > lines.len() {
            break;
        }
        if lines[i..i + pattern.len()]
            .iter()
            .zip(pattern)
            .all(|(l, p)| l.trim() == p.trim())
        {
            return Some(i);
        }
    }
    None
}

fn normalize_unicode(text: &str) -> String {
    let mut result = text.to_string();

    let replacements: Vec<(char, char)> = vec![
        ('\u{2013}', '-'),  // en dash
        ('\u{2014}', '-'),  // em dash
        ('\u{2015}', '-'),  // horizontal bar
        ('\u{2010}', '-'),  // hyphen
        ('\u{2011}', '-'),  // non-breaking hyphen
        ('\u{2012}', '-'),  // figure dash
        ('\u{2013}', '-'),  // en dash
        ('\u{2014}', '-'),  // em dash
        ('\u{2015}', '-'),  // horizontal bar
        ('\u{201C}', '"'),  // left double quotation mark
        ('\u{201D}', '"'),  // right double quotation mark
        ('\u{2018}', '\''), // left single quotation mark
        ('\u{2019}', '\''), // right single quotation mark
        ('\u{201A}', '\''), // single low-9 quotation mark
        ('\u{201B}', '\''), // single high-reversed-9 quotation mark
        ('\u{2018}', '\''), // left single quotation mark
        ('\u{2019}', '\''), // right single quotation mark
        ('\u{201C}', '"'),  // left double quotation mark
        ('\u{201D}', '"'),  // right double quotation mark
        ('\u{201E}', '"'),  // double low-9 quotation mark
        ('\u{201F}', '"'),  // double high-reversed-9 quotation mark
        ('\u{201C}', '"'),  // left double quotation mark
        ('\u{201D}', '"'),  // right double quotation mark
        ('\u{00A0}', ' '),  // non-breaking space
    ];

    for (from, to) in replacements {
        result = result.replace(from, &to.to_string());
    }

    result
}

fn find_subsequence_normalized(
    lines: &[String],
    start: usize,
    pattern: &[String],
    max_start: usize,
) -> Option<usize> {
    let end = max_start.min(lines.len());
    let normalized_pattern: Vec<String> = pattern.iter().map(|p| normalize_unicode(p)).collect();

    for i in start..end {
        if i + pattern.len() > lines.len() {
            break;
        }
        let normalized_lines: Vec<String> = lines[i..i + pattern.len()]
            .iter()
            .map(|l| normalize_unicode(l))
            .collect();
        if normalized_lines
            .iter()
            .zip(&normalized_pattern)
            .all(|(l, p)| l == p)
        {
            return Some(i);
        }
    }
    None
}

pub fn display_rel_path(cwd: &Path, path: &Path) -> String {
    path.strip_prefix(cwd).unwrap_or(path).display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_subsequence_empty_needle() {
        let haystack = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let result = find_subsequence(&haystack, 0, &[]);
        assert_eq!(result, Some(0));
    }

    #[test]
    fn find_subsequence_found() {
        let haystack = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let needle = vec!["b".to_string(), "c".to_string()];
        let result = find_subsequence(&haystack, 0, &needle);
        assert_eq!(result, Some(1));
    }

    #[test]
    fn find_subsequence_not_found() {
        let haystack = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let needle = vec!["d".to_string()];
        let result = find_subsequence(&haystack, 0, &needle);
        assert_eq!(result, None);
    }

    #[test]
    fn find_subsequence_start_offset() {
        let haystack = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let needle = vec!["b".to_string()];
        let result = find_subsequence(&haystack, 1, &needle);
        assert_eq!(result, Some(1));
    }

    #[test]
    fn display_rel_path_relative() {
        let cwd = Path::new("/workspace");
        let path = Path::new("/workspace/src/file.txt");
        let result = display_rel_path(cwd, path);
        assert_eq!(result, "src/file.txt");
    }

    #[test]
    fn display_rel_path_absolute() {
        let cwd = Path::new("/workspace");
        let path = Path::new("/other/file.txt");
        let result = display_rel_path(cwd, path);
        assert_eq!(result, "/other/file.txt");
    }
}
