use crate::tools::apply_patch::{parse_apply_patch, plan_patch_actions, apply_patch_actions};
use crate::tools::context::ToolTurnContext;
use crate::tools::error::FunctionCallError;
use std::path::Path;

pub async fn intercept_apply_patch(
    command: &[String],
    cwd: &Path,
    _timeout_ms: Option<u64>,
    _turn_context: &ToolTurnContext,
    _call_id: &str,
    _tool_name: &str,
) -> Result<Option<String>, FunctionCallError> {
    if command.len() < 2 || command[0] != "apply_patch" {
        return Ok(None);
    }

    let patch_input = command[1..].join(" ");

    match parse_apply_patch(&patch_input) {
        Ok(operations) => {
            match plan_patch_actions(cwd, &operations).await {
                Ok(actions) => {
                    match apply_patch_actions(&actions).await {
                        Ok(_) => {
                            tracing::warn!(
                                "apply_patch was requested via exec_command. Use apply_patch tool instead."
                            );
                            let summary = actions
                                .iter()
                                .map(|action| action.summary(cwd))
                                .collect::<Vec<String>>()
                                .join("\n");
                            Ok(Some(format!("patch applied successfully\n{summary}")))
                        }
                        Err(e) => Err(FunctionCallError::Internal(e.to_string())),
                    }
                }
                Err(e) => Err(FunctionCallError::Internal(e.to_string())),
            }
        }
        Err(e) => Err(FunctionCallError::Internal(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_intercept_non_apply_patch_command() {
        let temp = tempdir().expect("create tempdir");
        let cwd = temp.path();

        let command = vec!["echo".to_string(), "hello".to_string()];
        let result = intercept_apply_patch(
            &command,
            cwd,
            None,
            &ToolTurnContext::default(),
            "test-call-id",
            "test-tool",
        ).await.unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_intercept_apply_patch_command() {
        let temp = tempdir().expect("create tempdir");
        let cwd = temp.path();

        let patch = r#"*** Begin Patch
*** Add File: test.txt
+Hello world
*** End Patch"#;

        let command = vec!["apply_patch".to_string(), patch.to_string()];
        let result = intercept_apply_patch(
            &command,
            cwd,
            None,
            &ToolTurnContext::default(),
            "test-call-id",
            "test-tool",
        ).await.unwrap();

        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_intercept_apply_patch_with_valid_patch() {
        let temp = tempdir().expect("create tempdir");
        let cwd = temp.path();

        let patch = r#"*** Begin Patch
*** Add File: test.txt
+Hello world
*** End Patch"#;

        let command = vec!["apply_patch".to_string(), patch.to_string()];
        let result = intercept_apply_patch(
            &command,
            cwd,
            None,
            &ToolTurnContext::default(),
            "test-call-id",
            "test-tool",
        ).await.unwrap();

        assert!(result.is_some());
    }
}