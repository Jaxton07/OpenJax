use std::path::Path;

use super::matcher::display_rel_path;
use super::types::{HunkWarning, PlannedAction};

/// Number of context lines shown around each edited region in tool responses.
/// Provides the model with enough surrounding code to verify edits and
/// reason about line numbers for subsequent operations without a Read call.
pub const EDIT_CONTEXT_LINES: usize = 10;

/// Build a rich response string for apply_patch, showing per-action context snippets.
pub fn build_patch_response(actions: &[PlannedAction], cwd: &Path) -> String {
    let mut parts: Vec<String> = Vec::new();

    for action in actions {
        match action {
            PlannedAction::Update {
                path,
                content,
                changed_ranges,
                warnings,
            } => {
                let rel = display_rel_path(cwd, path);
                let lines: Vec<&str> = content.split('\n').collect();
                let total = lines.len();

                // Compute +add/-del counts from changed_ranges vs original.
                // We report the stats in the header line.
                let mut header = format!("UPDATE {rel}  ({total} lines total)");

                // Emit fuzzy/ambiguous warnings before context snippets.
                let mut warning_lines: Vec<String> = Vec::new();
                for w in warnings {
                    match w {
                        HunkWarning::FuzzyMatch { hunk_index, level } => {
                            let strategy = match level {
                                1 => "trailing whitespace ignored",
                                2 => "leading+trailing whitespace ignored",
                                3 => "unicode normalization applied",
                                _ => "fuzzy",
                            };
                            warning_lines.push(format!(
                                "  hunk {}: matched at fuzzy level {level} ({strategy}). \
                                 Verify the context snippet below is correct.",
                                hunk_index + 1
                            ));
                        }
                        HunkWarning::AmbiguousMatch { hunk_index } => {
                            warning_lines.push(format!(
                                "  hunk {}: context matched multiple locations, \
                                 applied to first occurrence. \
                                 Add more surrounding context to make the match unambiguous.",
                                hunk_index + 1
                            ));
                        }
                    }
                }
                if !warning_lines.is_empty() {
                    header.push('\n');
                    header.push_str(&warning_lines.join("\n"));
                }

                // Merge overlapping/adjacent windows for changed_ranges.
                let snippets = build_context_snippets(&lines, changed_ranges, EDIT_CONTEXT_LINES);

                let mut block = header;
                for snippet in snippets {
                    block.push('\n');
                    block.push_str(&snippet);
                }
                parts.push(block);
            }
            PlannedAction::Create { path, content } => {
                let rel = display_rel_path(cwd, path);
                let lines: Vec<&str> = content.split('\n').collect();
                let total = lines.len();
                let show = lines.len().min(EDIT_CONTEXT_LINES * 3);
                let mut block = format!("ADD {rel}  ({total} lines)");
                block.push('\n');
                block.push_str(&format_lines_with_numbers(&lines[..show], 1));
                parts.push(block);
            }
            PlannedAction::Delete { path } => {
                parts.push(format!("DELETE {}", display_rel_path(cwd, path)));
            }
            PlannedAction::Move { from, to } => {
                parts.push(format!(
                    "MOVE {} -> {}",
                    display_rel_path(cwd, from),
                    display_rel_path(cwd, to)
                ));
            }
        }
    }

    parts.join("\n")
}

/// Build a rich response for Edit, showing the edited region with context.
pub fn build_edit_response(
    file_path: &str,
    lines: &[String],
    edit_start: usize, // 1-indexed start of edited region
    edit_end: usize,   // 1-indexed end of edited region (inclusive), 0 if deleted to empty
    original_line_count: usize,
) -> String {
    let total = lines.len();
    let delta = total as i64 - original_line_count as i64;
    let delta_str = if delta >= 0 {
        format!("Δ+{delta}")
    } else {
        format!("Δ{delta}")
    };

    let header =
        format!("edit applied successfully\nUPDATE {file_path} (file: {total} lines, {delta_str})");

    if lines.is_empty() {
        return header;
    }

    let line_strs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();

    // Window around the edited region.
    let ctx_start = edit_start.saturating_sub(EDIT_CONTEXT_LINES + 1); // 0-indexed
    let ctx_end = (edit_end + EDIT_CONTEXT_LINES).min(total); // exclusive, 0-indexed

    let mut snippet = format!(
        "--- post-edit context (lines {}-{}) ---",
        ctx_start + 1,
        ctx_end
    );
    snippet.push('\n');

    for (i, line) in line_strs[ctx_start..ctx_end].iter().enumerate() {
        let line_num = ctx_start + i + 1;
        let in_edit = line_num >= edit_start && (edit_end == 0 || line_num <= edit_end);
        if in_edit {
            snippet.push_str(&format!("L{line_num}: {line}  <- edited\n"));
        } else {
            snippet.push_str(&format!("L{line_num}: {line}\n"));
        }
    }

    format!("{header}\n{snippet}")
}

/// Merge overlapping or closely adjacent hunk windows and render them as snippets.
fn build_context_snippets(
    lines: &[&str],
    changed_ranges: &[super::types::HunkResultRange],
    ctx: usize,
) -> Vec<String> {
    if changed_ranges.is_empty() {
        return Vec::new();
    }

    let total = lines.len();

    // Compute raw windows (0-indexed, exclusive end).
    let mut windows: Vec<(usize, usize, usize, usize)> = changed_ranges
        .iter()
        .map(|r| {
            let win_start = r.start.saturating_sub(ctx + 1); // 0-indexed
            let win_end = (r.end + ctx).min(total); // 0-indexed exclusive
            let edit_start = r.start - 1; // 0-indexed
            let edit_end = r.end.min(total); // 0-indexed exclusive
            (win_start, win_end, edit_start, edit_end)
        })
        .collect();

    // Merge adjacent windows (gap < ctx lines).
    let mut merged: Vec<(usize, usize, usize, usize)> = Vec::new();
    for (ws, we, es, ee) in windows.drain(..) {
        if let Some(last) = merged.last_mut()
            && ws <= last.1 + ctx
        {
            last.1 = last.1.max(we);
            last.3 = last.3.max(ee);
            continue;
        }
        merged.push((ws, we, es, ee));
    }

    merged
        .into_iter()
        .enumerate()
        .map(|(i, (ws, we, es, ee))| {
            let label = format!("--- hunk {} (lines {}-{}) ---", i + 1, ws + 1, we);
            let mut s = label;
            s.push('\n');
            s.push_str(&format_lines_with_edit_markers(
                &lines[ws..we],
                ws + 1,
                es,
                ee,
            ));
            s
        })
        .collect()
}

fn format_lines_with_numbers(lines: &[&str], start_line: usize) -> String {
    lines
        .iter()
        .enumerate()
        .map(|(i, line)| format!("L{}: {line}", start_line + i))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Format lines with `L{n}:` prefix. Lines in [edit_start, edit_end) (0-indexed) get no marker;
/// they are the edited region. The marker is omitted here — callers rely on the hunk header.
fn format_lines_with_edit_markers(
    lines: &[&str],
    start_line_num: usize,
    _edit_start: usize,
    _edit_end: usize,
) -> String {
    lines
        .iter()
        .enumerate()
        .map(|(i, line)| format!("L{}: {line}", start_line_num + i))
        .collect::<Vec<_>>()
        .join("\n")
}
