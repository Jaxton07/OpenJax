use crate::{HistoryEntry, MAX_TOOL_OUTPUT_CHARS_FOR_PROMPT};

pub(crate) fn truncate_for_prompt(text: &str) -> String {
    if text.chars().count() <= MAX_TOOL_OUTPUT_CHARS_FOR_PROMPT {
        return text.to_string();
    }

    let snippet = text
        .chars()
        .take(MAX_TOOL_OUTPUT_CHARS_FOR_PROMPT)
        .collect::<String>();
    format!("{snippet}...")
}

pub(crate) fn summarize_user_input(input: &str, preview_limit: usize) -> (String, bool) {
    let normalized = input.replace('\n', "\\n").replace('\r', "\\r");
    let total = normalized.chars().count();
    if total <= preview_limit {
        return (normalized, false);
    }

    let mut preview = normalized.chars().take(preview_limit).collect::<String>();
    preview.push_str("...");
    (preview, true)
}

pub(crate) fn build_planner_input(
    user_input: &str,
    history: &[HistoryEntry],
    tool_traces: &[String],
    remaining_calls: usize,
) -> String {
    let history_context = if history.is_empty() {
        "(no prior turns)".to_string()
    } else {
        history
            .iter()
            .map(|item| format!("{}: {}", item.role, item.content))
            .collect::<Vec<String>>()
            .join("\n")
    };

    let tool_context = if tool_traces.is_empty() {
        "(no tools executed yet)".to_string()
    } else {
        tool_traces.join("\n")
    };

    format!(
        "You are OpenJax's planning layer.\n\
Return ONLY valid JSON with one of two shapes:\n\
1) Tool call: {{\"action\":\"tool\",\"tool\":\"read_file|list_dir|grep_files|process_snapshot|system_load|disk_usage|shell|apply_patch|edit_file_range\",\"args\":{{...}}}}\n\
2) Final answer: {{\"action\":\"final\",\"message\":\"...\"}}\n\
\n\
Rules:\n\
- At most one action per response.\n\
- You can call tools up to {remaining_calls} more times this turn.\n\
- If task can be answered now, return final.\n\
- If action is final, message must be the direct, user-facing final answer (not a draft or meta explanation).\n\
- In final.message, avoid mentioning internal planning, hidden reasoning, or tool traces unless the user explicitly asks.\n\
- If required information is missing, use final.message to ask one concise clarification question.\n\
- IMPORTANT: All values inside args MUST be JSON strings (not numbers/booleans). Example: \"start_line\":\"6\".\n\
- For shell, put shell command in args.cmd.\n\
- Prefer process_snapshot/system_load/disk_usage for process and host metrics instead of shell ps/top/df commands when possible.\n\
- For apply_patch, use this EXACT format (note the space prefix for context lines):\n\
  *** Begin Patch\n\
  *** Update File: <filepath>\n\
  @@\n\
   context line (MUST start with space)\n\
  -line to remove (starts with -)\n\
  +line to add (starts with +)\n\
  *** End Patch\n\
  Operations: *** Add File:, *** Update File:, *** Delete File:, *** Move File: from -> to\n\
  IMPORTANT: In Update File, every line after @@ MUST start with space (context), - (remove), or + (add).\n\
  IMPORTANT: When modifying existing files, preserve the source file's formatting and style (indentation, line endings, spacing, quotes, trailing commas, and surrounding conventions).\n\
- For edit_file_range, provide args: file_path, start_line, end_line, new_text.\n\
- Tool selection policy:\n\
  - Prefer edit_file_range for single-file edits when exact line range is known.\n\
  - Prefer apply_patch for multi-file edits or file operations (add/delete/move/rename).\n\
  - If apply_patch fails with context mismatch (e.g., hunk context not found), call read_file before any further edits.\n\
  - For single-file follow-up fixes after that failure, prefer edit_file_range instead of retrying apply_patch on stale context.\n\
- IMPORTANT: Do NOT repeat the same tool call with the same arguments. Check the tool execution history carefully.\n\
- If a tool was already called and returned results, use those results to decide the next action.\n\
- Only call a tool again if you need different arguments or if the previous call failed.\n\
- After a successful apply_patch, do not call apply_patch again until you call read_file and inspect the latest file content.\n\
- After verification read_file, if the result already satisfies the request, return final immediately.\n\
- If verification already shows the requested content/changes are present, return final immediately.\n\
\n\
Conversation history (most recent last):\n{history_context}\n\
\n\
User request:\n{user_input}\n\
\n\
Tool execution history:\n{tool_context}\n"
    )
}

pub(crate) fn build_final_response_prompt(
    user_input: &str,
    tool_traces: &[String],
    seed_message: &str,
) -> String {
    let tool_context = if tool_traces.is_empty() {
        "(no tools executed in this turn)".to_string()
    } else {
        tool_traces.join("\n")
    };

    format!(
        "You are OpenJax's final response writer.\n\
Produce only the final assistant reply text for the user.\n\
Do not output JSON, markdown fences, or extra metadata.\n\
Keep the response concise, accurate, and actionable.\n\
\n\
User request:\n{user_input}\n\
\n\
Tool execution summary for this turn:\n{tool_context}\n\
\n\
Draft answer from planner:\n{seed_message}\n"
    )
}

pub(crate) fn build_json_repair_prompt(previous_output: &str) -> String {
    format!(
        "Your previous response did not match the required JSON schema.\n\
Return ONLY valid JSON. Do not include markdown, thoughts, or extra text.\n\
Allowed outputs:\n\
1) {{\"action\":\"tool\",\"tool\":\"read_file|list_dir|grep_files|process_snapshot|system_load|disk_usage|shell|apply_patch|edit_file_range\",\"args\":{{...}}}}\n\
2) {{\"action\":\"final\",\"message\":\"...\"}}\n\
\n\
Previous response:\n{previous_output}\n"
    )
}
