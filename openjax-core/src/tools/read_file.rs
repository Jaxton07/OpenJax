use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use std::collections::VecDeque;
use std::path::Path;
use tokio::io::{AsyncBufReadExt, BufReader};

use super::common::{parse_tool_args, take_bytes_at_char_boundary};
use crate::tools::ToolCall;

const READ_MAX_LINE_LENGTH: usize = 500;
const READ_TAB_WIDTH: usize = 4;
const READ_COMMENT_PREFIXES: &[&str] = &["#", "//", "--"];

#[derive(Deserialize)]
struct ReadFileArgs {
    #[serde(alias = "path", alias = "filepath")]
    file_path: String,
    #[serde(default = "read_default_offset")]
    offset: usize,
    #[serde(default = "read_default_limit")]
    limit: usize,
    #[serde(default)]
    mode: ReadMode,
    #[serde(default)]
    indentation: Option<IndentationArgs>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum ReadMode {
    #[default]
    Slice,
    Indentation,
}

#[derive(Deserialize, Clone, Default)]
struct IndentationArgs {
    #[serde(default)]
    anchor_line: Option<usize>,
    #[serde(default = "read_default_max_levels")]
    max_levels: usize,
    #[serde(default = "read_default_include_siblings")]
    include_siblings: bool,
    #[serde(default = "read_default_include_header")]
    include_header: bool,
    #[serde(default)]
    max_lines: Option<usize>,
}

#[derive(Clone, Debug)]
struct LineRecord {
    number: usize,
    raw: String,
    display: String,
    indent: usize,
}

impl LineRecord {
    fn trimmed(&self) -> &str {
        self.raw.trim_start()
    }

    fn is_blank(&self) -> bool {
        self.trimmed().is_empty()
    }

    fn is_comment(&self) -> bool {
        READ_COMMENT_PREFIXES
            .iter()
            .any(|prefix| self.raw.trim().starts_with(prefix))
    }
}

fn read_default_offset() -> usize { 1 }
fn read_default_limit() -> usize { 2000 }
fn read_default_max_levels() -> usize { 0 }
fn read_default_include_siblings() -> bool { false }
fn read_default_include_header() -> bool { true }

pub async fn read_file(call: &ToolCall, cwd: &Path) -> Result<String> {
    let args: ReadFileArgs = parse_tool_args(&call.args)?;

    if args.offset == 0 {
        return Err(anyhow!("offset must be a 1-indexed line number"));
    }

    if args.limit == 0 {
        return Err(anyhow!("limit must be greater than zero"));
    }

    let rel_path = args.file_path;
    let path = crate::tools::resolve_workspace_path(cwd, &rel_path)?;

    let collected = match args.mode {
        ReadMode::Slice => read_file_slice(&path, args.offset, args.limit).await?,
        ReadMode::Indentation => {
            let indentation = args.indentation.unwrap_or_default();
            read_file_indentation(&path, args.offset, args.limit, indentation).await?
        }
    };

    Ok(collected.join("\n"))
}

async fn read_file_slice(path: &Path, offset: usize, limit: usize) -> Result<Vec<String>> {
    let file = tokio::fs::File::open(path)
        .await
        .with_context(|| "failed to read file")?;

    let mut reader = BufReader::new(file);
    let mut collected = Vec::new();
    let mut seen = 0usize;
    let mut buffer = Vec::new();

    loop {
        buffer.clear();
        let bytes_read = reader.read_until(b'\n', &mut buffer).await
            .with_context(|| "failed to read file")?;

        if bytes_read == 0 { break; }

        if buffer.last() == Some(&b'\n') {
            buffer.pop();
            if buffer.last() == Some(&b'\r') {
                buffer.pop();
            }
        }

        seen += 1;

        if seen < offset { continue; }
        if collected.len() == limit { break; }

        let formatted = format_read_line(&buffer);
        collected.push(format!("L{seen}: {formatted}"));
    }

    if seen < offset {
        return Err(anyhow!("offset exceeds file length"));
    }

    Ok(collected)
}

async fn read_file_indentation(
    path: &Path,
    offset: usize,
    limit: usize,
    options: IndentationArgs,
) -> Result<Vec<String>> {
    let anchor_line = options.anchor_line.unwrap_or(offset);
    let guard_limit = options.max_lines.unwrap_or(limit);

    let collected = collect_file_lines(path).await?;
    if collected.is_empty() || anchor_line > collected.len() {
        return Err(anyhow!("anchor_line exceeds file length"));
    }

    let anchor_index = anchor_line - 1;
    let effective_indents = compute_effective_indents(&collected);
    let anchor_indent = effective_indents[anchor_index];

    let min_indent = if options.max_levels == 0 {
        0
    } else {
        anchor_indent.saturating_sub(options.max_levels * READ_TAB_WIDTH)
    };

    let final_limit = limit.min(guard_limit).min(collected.len());

    if final_limit == 1 {
        return Ok(vec![format!(
            "L{}: {}",
            collected[anchor_index].number, collected[anchor_index].display
        )]);
    }

    let mut i: isize = anchor_index as isize - 1;
    let mut j: usize = anchor_index + 1;
    let mut i_counter_min_indent = 0;
    let mut j_counter_min_indent = 0;

    let mut out = VecDeque::with_capacity(limit);
    out.push_back(&collected[anchor_index]);

    while out.len() < final_limit {
        let mut progressed = 0;

        if i >= 0 {
            let iu = i as usize;
            if effective_indents[iu] >= min_indent {
                out.push_front(&collected[iu]);
                progressed += 1;
                i -= 1;

                if effective_indents[iu] == min_indent && !options.include_siblings {
                    let allow_header_comment =
                        options.include_header && collected[iu].is_comment();
                    let can_take_line = allow_header_comment || i_counter_min_indent == 0;

                    if can_take_line {
                        i_counter_min_indent += 1;
                    } else {
                        out.pop_front();
                        progressed -= 1;
                        i = -1;
                    }
                }

                if out.len() >= final_limit { break; }
            } else {
                i = -1;
            }
        }

        if j < collected.len() {
            let ju = j;
            if effective_indents[ju] >= min_indent {
                out.push_back(&collected[ju]);
                progressed += 1;
                j += 1;

                if effective_indents[ju] == min_indent && !options.include_siblings {
                    if j_counter_min_indent > 0 {
                        out.pop_back();
                        progressed -= 1;
                        j = collected.len();
                    }
                    j_counter_min_indent += 1;
                }
            } else {
                j = collected.len();
            }
        }

        if progressed == 0 { break; }
    }

    trim_empty_lines(&mut out);

    Ok(out.into_iter()
        .map(|record| format!("L{}: {}", record.number, record.display))
        .collect())
}

async fn collect_file_lines(path: &Path) -> Result<Vec<LineRecord>> {
    let file = tokio::fs::File::open(path)
        .await
        .with_context(|| "failed to read file")?;

    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();
    let mut lines = Vec::new();
    let mut number = 0usize;

    loop {
        buffer.clear();
        let bytes_read = reader.read_until(b'\n', &mut buffer).await
            .with_context(|| "failed to read file")?;

        if bytes_read == 0 { break; }

        if buffer.last() == Some(&b'\n') {
            buffer.pop();
            if buffer.last() == Some(&b'\r') {
                buffer.pop();
            }
        }

        number += 1;
        let raw = String::from_utf8_lossy(&buffer).into_owned();
        let indent = measure_indent(&raw);
        let display = format_read_line(&buffer);
        lines.push(LineRecord { number, raw, display, indent });
    }

    Ok(lines)
}

fn compute_effective_indents(records: &[LineRecord]) -> Vec<usize> {
    let mut effective = Vec::with_capacity(records.len());
    let mut previous_indent = 0usize;
    for record in records {
        if record.is_blank() {
            effective.push(previous_indent);
        } else {
            previous_indent = record.indent;
            effective.push(previous_indent);
        }
    }
    effective
}

fn measure_indent(line: &str) -> usize {
    line.chars()
        .take_while(|c| matches!(c, ' ' | '\t'))
        .map(|c| if c == '\t' { READ_TAB_WIDTH } else { 1 })
        .sum()
}

fn format_read_line(bytes: &[u8]) -> String {
    let decoded = String::from_utf8_lossy(bytes);
    if decoded.len() > READ_MAX_LINE_LENGTH {
        take_bytes_at_char_boundary(&decoded, READ_MAX_LINE_LENGTH).to_string()
    } else {
        decoded.into_owned()
    }
}

fn trim_empty_lines(out: &mut VecDeque<&LineRecord>) {
    while matches!(out.front(), Some(line) if line.raw.trim().is_empty()) {
        out.pop_front();
    }
    while matches!(out.back(), Some(line) if line.raw.trim().is_empty()) {
        out.pop_back();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[tokio::test]
    async fn reads_requested_range() {
        let mut temp = NamedTempFile::new().unwrap();
        write!(temp, "alpha\nbeta\ngamma\n").unwrap();

        let lines = read_file_slice(temp.path(), 2, 2).await.unwrap();
        assert_eq!(lines, vec!["L2: beta".to_string(), "L3: gamma".to_string()]);
    }

    #[tokio::test]
    async fn errors_when_offset_exceeds_length() {
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, "only").unwrap();

        let err = read_file_slice(temp.path(), 3, 1).await.expect_err("offset exceeds length");
        assert_eq!(err.to_string(), "offset exceeds file length");
    }

    #[tokio::test]
    async fn reads_non_utf8_lines() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.as_file_mut().write_all(b"\xff\xfe\nplain\n").unwrap();

        let lines = read_file_slice(temp.path(), 1, 2).await.unwrap();
        let expected_first = format!("L1: {}{}", '\u{FFFD}', '\u{FFFD}');
        assert_eq!(lines, vec![expected_first, "L2: plain".to_string()]);
    }

    #[tokio::test]
    async fn trims_crlf_endings() {
        let mut temp = NamedTempFile::new().unwrap();
        write!(temp, "one\r\ntwo\r\n").unwrap();

        let lines = read_file_slice(temp.path(), 1, 2).await.unwrap();
        assert_eq!(lines, vec!["L1: one".to_string(), "L2: two".to_string()]);
    }

    #[tokio::test]
    async fn respects_limit_even_with_more_lines() {
        let mut temp = NamedTempFile::new().unwrap();
        write!(temp, "first\nsecond\nthird\n").unwrap();

        let lines = read_file_slice(temp.path(), 1, 2).await.unwrap();
        assert_eq!(
            lines,
            vec!["L1: first".to_string(), "L2: second".to_string()]
        );
    }

    #[tokio::test]
    async fn truncates_lines_longer_than_max_length() {
        let mut temp = NamedTempFile::new().unwrap();
        let long_line = "x".repeat(READ_MAX_LINE_LENGTH + 50);
        writeln!(temp, "{long_line}").unwrap();

        let lines = read_file_slice(temp.path(), 1, 1).await.unwrap();
        let expected = "x".repeat(READ_MAX_LINE_LENGTH);
        assert_eq!(lines, vec![format!("L1: {expected}")]);
    }

    #[tokio::test]
    async fn indentation_mode_captures_block() {
        let mut temp = NamedTempFile::new().unwrap();
        write!(
            temp,
            "fn outer() {{
    if cond {{
        inner();
    }}
    tail();
}}
"
        ).unwrap();

        let options = IndentationArgs {
            anchor_line: Some(3),
            include_siblings: false,
            max_levels: 1,
            ..Default::default()
        };

        let lines = read_file_indentation(temp.path(), 3, 10, options).await.unwrap();

        assert_eq!(
            lines,
            vec![
                "L2:     if cond {".to_string(),
                "L3:         inner();".to_string(),
                "L4:     }".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn indentation_mode_expands_parents() {
        let mut temp = NamedTempFile::new().unwrap();
        write!(
            temp,
            "mod root {{
    fn outer() {{
        if cond {{
            inner();
        }}
    }}
}}
"
        ).unwrap();

        let mut options = IndentationArgs {
            anchor_line: Some(4),
            max_levels: 2,
            ..Default::default()
        };

        let lines = read_file_indentation(temp.path(), 4, 50, options.clone()).await.unwrap();
        assert_eq!(
            lines,
            vec![
                "L2:     fn outer() {".to_string(),
                "L3:         if cond {".to_string(),
                "L4:             inner();".to_string(),
                "L5:         }".to_string(),
                "L6:     }".to_string(),
            ]
        );

        options.max_levels = 3;
        let expanded = read_file_indentation(temp.path(), 4, 50, options).await.unwrap();
        assert_eq!(
            expanded,
            vec![
                "L1: mod root {".to_string(),
                "L2:     fn outer() {".to_string(),
                "L3:         if cond {".to_string(),
                "L4:             inner();".to_string(),
                "L5:         }".to_string(),
                "L6:     }".to_string(),
                "L7: }".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn indentation_mode_respects_sibling_flag() {
        let mut temp = NamedTempFile::new().unwrap();
        write!(
            temp,
            "fn wrapper() {{
    if first {{
        do_first();
    }}
    if second {{
        do_second();
    }}
}}
"
        ).unwrap();

        let mut options = IndentationArgs {
            anchor_line: Some(3),
            include_siblings: false,
            max_levels: 1,
            ..Default::default()
        };

        let lines = read_file_indentation(temp.path(), 3, 50, options.clone()).await.unwrap();
        assert_eq!(
            lines,
            vec![
                "L2:     if first {".to_string(),
                "L3:         do_first();".to_string(),
                "L4:     }".to_string(),
            ]
        );

        options.include_siblings = true;
        let with_siblings = read_file_indentation(temp.path(), 3, 50, options).await.unwrap();
        assert_eq!(
            with_siblings,
            vec![
                "L2:     if first {".to_string(),
                "L3:         do_first();".to_string(),
                "L4:     }".to_string(),
                "L5:     if second {".to_string(),
                "L6:         do_second();".to_string(),
                "L7:     }".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn indentation_mode_handles_python_sample() {
        let mut temp = NamedTempFile::new().unwrap();
        write!(
            temp,
            "class Foo:
    def __init__(self, size):
        self.size = size
    def double(self, value):
        if value is None:
            return 0
        result = value * self.size
        return result
class Bar:
    def compute(self):
        helper = Foo(2)
        return helper.double(5)
"
        ).unwrap();

        let options = IndentationArgs {
            anchor_line: Some(7),
            include_siblings: true,
            max_levels: 1,
            ..Default::default()
        };

        let lines = read_file_indentation(temp.path(), 1, 200, options).await.unwrap();
        assert_eq!(
            lines,
            vec![
                "L2:     def __init__(self, size):".to_string(),
                "L3:         self.size = size".to_string(),
                "L4:     def double(self, value):".to_string(),
                "L5:         if value is None:".to_string(),
                "L6:             return 0".to_string(),
                "L7:         result = value * self.size".to_string(),
                "L8:         return result".to_string(),
            ]
        );
    }

    fn write_cpp_sample() -> anyhow::Result<NamedTempFile> {
        let mut temp = NamedTempFile::new()?;
        write!(
            temp,
            "#include <vector>
#include <string>

namespace sample {{
class Runner {{
public:
    void setup() {{
        if (enabled_) {{
            init();
        }}
    }}

    // Run the code
    int run() const {{
        switch (mode_) {{
            case Mode::Fast:
                return fast();
            case Mode::Slow:
                return slow();
            default:
                return fallback();
        }}
    }}

private:
    bool enabled_ = false;
    Mode mode_ = Mode::Fast;

    int fast() const {{
        return 1;
    }}
}};
}}  // namespace sample
"
        )?;
        Ok(temp)
    }

    #[tokio::test]
    async fn indentation_mode_handles_cpp_sample_shallow() {
        let temp = write_cpp_sample().unwrap();

        let options = IndentationArgs {
            include_siblings: false,
            anchor_line: Some(18),
            max_levels: 1,
            ..Default::default()
        };

        let lines = read_file_indentation(temp.path(), 18, 200, options).await.unwrap();
        assert_eq!(
            lines,
            vec![
                "L15:         switch (mode_) {".to_string(),
                "L16:             case Mode::Fast:".to_string(),
                "L17:                 return fast();".to_string(),
                "L18:             case Mode::Slow:".to_string(),
                "L19:                 return slow();".to_string(),
                "L20:             default:".to_string(),
                "L21:                 return fallback();".to_string(),
                "L22:         }".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn indentation_mode_handles_cpp_sample() {
        let temp = write_cpp_sample().unwrap();

        let options = IndentationArgs {
            include_siblings: false,
            include_header: true,
            anchor_line: Some(18),
            max_levels: 2,
            ..Default::default()
        };

        let lines = read_file_indentation(temp.path(), 18, 200, options).await.unwrap();
        assert_eq!(
            lines,
            vec![
                "L13:     // Run the code".to_string(),
                "L14:     int run() const {".to_string(),
                "L15:         switch (mode_) {".to_string(),
                "L16:             case Mode::Fast:".to_string(),
                "L17:                 return fast();".to_string(),
                "L18:             case Mode::Slow:".to_string(),
                "L19:                 return slow();".to_string(),
                "L20:             default:".to_string(),
                "L21:                 return fallback();".to_string(),
                "L22:         }".to_string(),
                "L23:     }".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn indentation_mode_handles_cpp_sample_no_headers() {
        let temp = write_cpp_sample().unwrap();

        let options = IndentationArgs {
            include_siblings: false,
            include_header: false,
            anchor_line: Some(18),
            max_levels: 2,
            ..Default::default()
        };

        let lines = read_file_indentation(temp.path(), 18, 200, options).await.unwrap();
        assert_eq!(
            lines,
            vec![
                "L14:     int run() const {".to_string(),
                "L15:         switch (mode_) {".to_string(),
                "L16:             case Mode::Fast:".to_string(),
                "L17:                 return fast();".to_string(),
                "L18:             case Mode::Slow:".to_string(),
                "L19:                 return slow();".to_string(),
                "L20:             default:".to_string(),
                "L21:                 return fallback();".to_string(),
                "L22:         }".to_string(),
                "L23:     }".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn indentation_mode_handles_cpp_sample_siblings() {
        let temp = write_cpp_sample().unwrap();

        let options = IndentationArgs {
            include_siblings: true,
            include_header: false,
            anchor_line: Some(18),
            max_levels: 2,
            ..Default::default()
        };

        let lines = read_file_indentation(temp.path(), 18, 200, options).await.unwrap();
        assert_eq!(
            lines,
            vec![
                "L7:     void setup() {".to_string(),
                "L8:         if (enabled_) {".to_string(),
                "L9:             init();".to_string(),
                "L10:         }".to_string(),
                "L11:     }".to_string(),
                "L12: ".to_string(),
                "L13:     // Run the code".to_string(),
                "L14:     int run() const {".to_string(),
                "L15:         switch (mode_) {".to_string(),
                "L16:             case Mode::Fast:".to_string(),
                "L17:                 return fast();".to_string(),
                "L18:             case Mode::Slow:".to_string(),
                "L19:                 return slow();".to_string(),
                "L20:             default:".to_string(),
                "L21:                 return fallback();".to_string(),
                "L22:         }".to_string(),
                "L23:     }".to_string(),
            ]
        );
    }
}
