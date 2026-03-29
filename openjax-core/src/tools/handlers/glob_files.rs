use async_trait::async_trait;
use glob::glob;
use serde::Deserialize;
use serde::de::{self, Deserializer};
use std::path::Path;
use std::time::SystemTime;

use crate::tools::context::{FunctionCallOutputBody, ToolInvocation, ToolOutput, ToolPayload};
use crate::tools::error::FunctionCallError;
use crate::tools::registry::{ToolHandler, ToolKind};

const GLOB_DEFAULT_LIMIT: usize = 100;
const GLOB_MAX_LIMIT: usize = 2000;

#[derive(Deserialize)]
struct GlobFilesArgs {
    pattern: String,
    #[serde(default)]
    base_path: Option<String>,
    #[serde(default = "glob_default_limit", deserialize_with = "de_usize")]
    limit: usize,
}

#[derive(Debug)]
struct GlobMatch {
    relative_path: String,
    modified: SystemTime,
}

fn glob_default_limit() -> usize {
    GLOB_DEFAULT_LIMIT
}

fn de_usize<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NumOrString {
        Num(usize),
        Str(String),
    }

    match NumOrString::deserialize(deserializer)? {
        NumOrString::Num(n) => Ok(n),
        NumOrString::Str(s) => s
            .trim()
            .parse::<usize>()
            .map_err(|_| de::Error::custom("expected positive integer")),
    }
}

pub struct GlobFilesHandler;

#[async_trait]
impl ToolHandler for GlobFilesHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation { payload, turn, .. } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "glob_files handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: GlobFilesArgs = serde_json::from_str(&arguments).map_err(|e| {
            FunctionCallError::Internal(format!("failed to parse arguments: {}", e))
        })?;

        let pattern = args.pattern.trim();
        if pattern.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "pattern must not be empty".to_string(),
            ));
        }
        if Path::new(pattern).is_absolute() {
            return Err(FunctionCallError::RespondToModel(
                "pattern must be a relative glob".to_string(),
            ));
        }
        if crate::tools::contains_parent_dir(Path::new(pattern)) {
            return Err(FunctionCallError::RespondToModel(format!(
                "path escapes workspace: parent traversal is not allowed ({pattern})"
            )));
        }
        if args.limit == 0 {
            return Err(FunctionCallError::RespondToModel(
                "limit must be greater than zero".to_string(),
            ));
        }

        let limit = args.limit.min(GLOB_MAX_LIMIT);
        let base_path = args.base_path.unwrap_or_else(|| ".".to_string());
        let base_dir = crate::tools::resolve_workspace_path(&turn.cwd, &base_path)
            .map_err(|e| FunctionCallError::Internal(e.to_string()))?;
        let workspace_root = turn.cwd.canonicalize().map_err(|e| {
            FunctionCallError::Internal(format!("failed to canonicalize workspace root: {}", e))
        })?;

        let joined_pattern = base_dir.join(pattern);
        let joined_pattern = joined_pattern.to_string_lossy().replace('\\', "/");
        let mut matches = Vec::new();

        for entry in glob(&joined_pattern).map_err(|e| {
            FunctionCallError::Internal(format!("failed to parse glob pattern: {e}"))
        })? {
            let matched = entry.map_err(|e| {
                FunctionCallError::Internal(format!("failed to evaluate glob pattern: {e}"))
            })?;
            let resolved = matched.canonicalize().map_err(|e| {
                FunctionCallError::Internal(format!(
                    "failed to canonicalize matched path `{}`: {e}",
                    matched.display()
                ))
            })?;

            if !resolved.starts_with(&workspace_root) {
                return Err(FunctionCallError::Internal(format!(
                    "path escapes workspace: {}",
                    resolved.display()
                )));
            }

            let metadata = tokio::fs::metadata(&resolved).await.map_err(|e| {
                FunctionCallError::Internal(format!(
                    "failed to read metadata for `{}`: {e}",
                    resolved.display()
                ))
            })?;
            if !metadata.is_file() {
                continue;
            }

            let relative = resolved.strip_prefix(&workspace_root).map_err(|e| {
                FunctionCallError::Internal(format!(
                    "failed to resolve workspace-relative path for `{}`: {e}",
                    resolved.display()
                ))
            })?;
            let relative_path = relative.to_string_lossy().replace('\\', "/");
            let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            matches.push(GlobMatch {
                relative_path,
                modified,
            });
        }

        matches.sort_by(|a, b| {
            b.modified
                .cmp(&a.modified)
                .then_with(|| a.relative_path.cmp(&b.relative_path))
        });
        matches.truncate(limit);

        let output = matches
            .into_iter()
            .map(|entry| entry.relative_path)
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(output),
            success: Some(true),
        })
    }
}
