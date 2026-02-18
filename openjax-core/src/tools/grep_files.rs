use anyhow::{Result, anyhow};
use serde::Deserialize;
use std::path::Path;
use tokio::process::Command;
use tokio::time::{Duration, timeout};

use super::common::{parse_tool_args, verify_path_exists};
use crate::tools::ToolCall;

const GREP_DEFAULT_LIMIT: usize = 100;
const GREP_MAX_LIMIT: usize = 2000;
const GREP_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Deserialize)]
struct GrepFilesArgs {
    pattern: String,
    #[serde(default)]
    include: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default = "grep_default_limit")]
    limit: usize,
}

fn grep_default_limit() -> usize {
    GREP_DEFAULT_LIMIT
}

pub async fn grep_files(call: &ToolCall, cwd: &Path) -> Result<String> {
    let args: GrepFilesArgs = parse_tool_args(&call.args)?;

    let pattern = args.pattern.trim();
    if pattern.is_empty() {
        return Err(anyhow!("pattern must not be empty"));
    }

    if args.limit == 0 {
        return Err(anyhow!("limit must be greater than zero"));
    }

    let limit = args.limit.min(GREP_MAX_LIMIT);
    let rel_path = args.path.unwrap_or_else(|| ".".to_string());
    let search_path = crate::tools::resolve_workspace_path(cwd, &rel_path)?;

    verify_path_exists(&search_path).await?;

    let include = args.include.as_deref().map(str::trim).and_then(|val| {
        if val.is_empty() { None } else { Some(val.to_string()) }
    });

    let search_results = run_rg_search(pattern, include.as_deref(), &search_path, limit, cwd).await?;

    if search_results.is_empty() {
        Ok("No matches found.".to_string())
    } else {
        Ok(search_results.join("\n"))
    }
}

async fn run_rg_search(
    pattern: &str,
    include: Option<&str>,
    search_path: &Path,
    limit: usize,
    cwd: &Path,
) -> Result<Vec<String>> {
    let mut command = Command::new("rg");
    command
        .current_dir(cwd)
        .arg("--files-with-matches")
        .arg("--sortr=modified")
        .arg("--regexp")
        .arg(pattern)
        .arg("--no-messages");

    if let Some(glob) = include {
        command.arg("--glob").arg(glob);
    }

    command.arg("--").arg(search_path);

    let output = timeout(GREP_COMMAND_TIMEOUT, command.output())
        .await
        .map_err(|_| anyhow!("rg timed out after 30 seconds"))?
        .map_err(|err| anyhow!("failed to launch rg: {err}. Ensure ripgrep is installed and on PATH."))?;

    match output.status.code() {
        Some(0) => Ok(parse_rg_results(&output.stdout, limit)),
        Some(1) => Ok(Vec::new()),
        _ => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!("rg failed: {stderr}"))
        }
    }
}

fn parse_rg_results(stdout: &[u8], limit: usize) -> Vec<String> {
    let mut results = Vec::new();
    for line in stdout.split(|byte| *byte == b'\n') {
        if line.is_empty() { continue; }
        if let Ok(text) = std::str::from_utf8(line) {
            if text.is_empty() { continue; }
            results.push(text.to_string());
            if results.len() == limit { break; }
        }
    }
    results
}
