use anyhow::{Result, anyhow};
use std::path::Path;

use super::applier::apply_patch_actions;
use super::heredoc::normalize_patch_arg;
use super::parser::parse_apply_patch;
use super::planner::plan_patch_actions;
use crate::tools::ToolCall;

pub async fn apply_patch_tool(call: &ToolCall, cwd: &Path) -> Result<String> {
    let patch_arg = call
        .args
        .get("patch")
        .ok_or_else(|| anyhow!("apply_patch requires patch='<patch text>'"))?;
    let normalized_patch = normalize_patch_arg(patch_arg);
    let operations = parse_apply_patch(&normalized_patch)?;
    let actions = plan_patch_actions(cwd, &operations).await?;
    apply_patch_actions(&actions).await?;

    let summary = actions
        .iter()
        .map(|action| action.summary(cwd))
        .collect::<Vec<String>>()
        .join("\n");

    Ok(format!("patch applied successfully\n{summary}"))
}
