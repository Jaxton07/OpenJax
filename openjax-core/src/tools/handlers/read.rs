use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::VecDeque;
use std::path::Path;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::debug;

use super::de_helpers;

use crate::tools::context::{FunctionCallOutputBody, ToolInvocation, ToolOutput, ToolPayload};
use crate::tools::error::FunctionCallError;
use crate::tools::registry::{ToolHandler, ToolKind};

const READ_MAX_LINE_LENGTH: usize = 500;
const READ_TAB_WIDTH: usize = 4;
const READ_COMMENT_PREFIXES: &[&str] = &["#", "//", "--"];

#[derive(Deserialize)]
struct ReadFileArgs {
    #[serde(alias = "path", alias = "filepath")]
    file_path: String,
    #[serde(default = "read_default_offset", deserialize_with = "de_helpers::de_usize")]
    offset: usize,
    #[serde(default = "read_default_limit", deserialize_with = "de_helpers::de_usize")]
    limit: usize,
    #[serde(default)]
    mode: ReadMode,
    #[serde(default)]
    indentation: Option<IndentationArgs>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "snake_case")]
enum ReadMode {
    #[default]
    Slice,
    Indentation,
}

#[derive(Deserialize, Clone, Default)]
struct IndentationArgs {
    #[serde(default, deserialize_with = "de_helpers::de_opt_usize")]
    anchor_line: Option<usize>,
    #[serde(default = "read_default_max_levels", deserialize_with = "de_helpers::de_usize")]
    max_levels: usize,
    #[serde(default = "read_default_include_siblings")]
    include_siblings: bool,
    #[serde(default = "read_default_include_header")]
    include_header: bool,
    #[serde(default, deserialize_with = "de_helpers::de_opt_usize")]
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

fn read_default_offset() -> usize {
    1
}
fn read_default_limit() -> usize {
    2000
}
fn read_default_max_levels() -> usize {
    0
}
fn read_default_include_siblings() -> bool {
    false
}
fn read_default_include_header() -> bool {
    true
}

pub struct ReadHandler;

#[async_trait]
impl ToolHandler for ReadHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation { payload, turn, .. } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "Read handler received unsupported payload".to_string(),
                ));
            }
        };

        debug!(
            raw_arguments = %arguments,
            cwd = %turn.cwd.display(),
            "Read parsing arguments"
        );

        let args: ReadFileArgs = serde_json::from_str(&arguments).map_err(|e| {
            debug!(error = %e, raw_arguments = %arguments, "Read failed to parse arguments");
            FunctionCallError::Internal(format!("failed to parse arguments: {}", e))
        })?;

        debug!(
            file_path = %args.file_path,
            offset = args.offset,
            limit = args.limit,
            mode = ?args.mode,
            "Read parsed arguments"
        );

        if args.offset == 0 {
            return Err(FunctionCallError::RespondToModel(
                "offset must be a 1-indexed line number".to_string(),
            ));
        }

        if args.limit == 0 {
            return Err(FunctionCallError::RespondToModel(
                "limit must be greater than zero".to_string(),
            ));
        }

        let rel_path = args.file_path;
        let path = crate::tools::resolve_workspace_path(&turn.cwd, &rel_path)
            .map_err(|e| FunctionCallError::Internal(e.to_string()))?;

        let collected = match args.mode {
            ReadMode::Slice => read_slice(&path, args.offset, args.limit)
                .await
                .map_err(|e| FunctionCallError::Internal(e.to_string()))?,
            ReadMode::Indentation => {
                let indentation = args.indentation.unwrap_or_default();
                read_indentation(&path, args.offset, args.limit, indentation)
                    .await
                    .map_err(|e| FunctionCallError::Internal(e.to_string()))?
            }
        };

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(collected.join("\n")),
            success: Some(true),
        })
    }
}

async fn read_slice(path: &Path, offset: usize, limit: usize) -> Result<Vec<String>> {
    let file = tokio::fs::File::open(path)
        .await
        .with_context(|| "failed to read file")?;

    let mut reader = BufReader::new(file);
    let mut collected = Vec::new();
    let mut seen = 0usize;
    let mut buffer = Vec::new();

    loop {
        buffer.clear();
        let bytes_read = reader
            .read_until(b'\n', &mut buffer)
            .await
            .with_context(|| "failed to read file")?;

        if bytes_read == 0 {
            break;
        }

        if buffer.last() == Some(&b'\n') {
            buffer.pop();
            if buffer.last() == Some(&b'\r') {
                buffer.pop();
            }
        }

        seen += 1;

        if seen < offset {
            continue;
        }
        if collected.len() == limit {
            break;
        }

        let formatted = format_read_line(&buffer);
        collected.push(format!("L{seen}: {formatted}"));
    }

    if seen < offset {
        return Err(anyhow!("offset exceeds file length"));
    }

    Ok(collected)
}

async fn read_indentation(
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
                    let allow_header_comment = options.include_header && collected[iu].is_comment();
                    let can_take_line = allow_header_comment || i_counter_min_indent == 0;

                    if can_take_line {
                        i_counter_min_indent += 1;
                    } else {
                        out.pop_front();
                        progressed -= 1;
                        i = -1;
                    }
                }

                if out.len() >= final_limit {
                    break;
                }
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

        if progressed == 0 {
            break;
        }
    }

    trim_empty_lines(&mut out);

    Ok(out
        .into_iter()
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
        let bytes_read = reader
            .read_until(b'\n', &mut buffer)
            .await
            .with_context(|| "failed to read file")?;

        if bytes_read == 0 {
            break;
        }

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
        lines.push(LineRecord {
            number,
            raw,
            display,
            indent,
        });
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
        crate::tools::take_bytes_at_char_boundary(&decoded, READ_MAX_LINE_LENGTH).to_string()
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
