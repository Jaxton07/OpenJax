use std::collections::HashMap;

use serde_json::Value;

use crate::agent::planner::ToolActionContext;
use crate::agent::planner_utils::{
    detect_diff_strategy, is_git_diff_stat, is_git_status_short,
    looks_like_skill_trigger_shell_command, merge_diff_strategy,
};
use crate::agent::prompt::truncate_for_prompt;

pub(super) fn observe_tool_args(args: &HashMap<String, String>, ctx: &mut ToolActionContext<'_>) {
    if let Some(cmd) = args.get("cmd")
        && looks_like_skill_trigger_shell_command(cmd)
    {
        *ctx.skill_shell_misfire_count = (*ctx.skill_shell_misfire_count).saturating_add(1);
    }
    if let Some(cmd) = args.get("cmd") {
        if is_git_status_short(cmd) {
            *ctx.saw_git_status_short = true;
        }
        if is_git_diff_stat(cmd) {
            *ctx.saw_git_diff_stat = true;
        }
        if let Some(next_strategy) = detect_diff_strategy(cmd) {
            *ctx.diff_strategy = merge_diff_strategy(ctx.diff_strategy, next_strategy);
        }
    }
}

pub(super) fn tool_trace<T: std::fmt::Display>(
    tool_name: &str,
    ok: T,
    output: &str,
    max_chars: usize,
) -> String {
    format!(
        "tool={tool_name}; ok={ok}; output={}",
        truncate_for_prompt(output, max_chars)
    )
}

pub(super) fn tool_trace_with_args(
    tool_name: &str,
    ok: &str,
    args: &HashMap<String, String>,
    output: &str,
    max_chars: usize,
) -> String {
    format!(
        "tool={tool_name}; ok={ok}; args={}; output={}",
        serde_json::to_string(args).unwrap_or_default(),
        truncate_for_prompt(output, max_chars)
    )
}

pub(super) fn tool_input_to_args(input: &Value) -> HashMap<String, String> {
    let mut args = HashMap::new();
    let Value::Object(map) = input else {
        return args;
    };
    for (key, value) in map {
        let stringified = match value {
            Value::String(s) => s.clone(),
            other => serde_json::to_string(other).unwrap_or_default(),
        };
        args.insert(key.clone(), stringified);
    }
    args
}
