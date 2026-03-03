use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use tracing::info;

use crate::approval::ApprovalRequest;
use crate::tools::apply_patch_interceptor;
use crate::tools::context::{FunctionCallOutputBody, ToolInvocation, ToolOutput, ToolPayload};
use crate::tools::error::FunctionCallError;
use crate::tools::policy::{PolicyDecision, evaluate_tool_invocation_policy};
use crate::tools::registry::{ToolHandler, ToolKind};
use crate::tools::sandbox_runtime::{
    SandboxDegradePolicy, SandboxExecutionRequest, SandboxRuntimeSettings, audit_log,
    ensure_workspace_relative_paths, execute_in_sandbox, run_without_sandbox,
};
use crate::tools::shell::{Shell, ShellType};
use crate::tools::{ApprovalPolicy, SandboxMode};
use openjax_protocol::Event;
use uuid::Uuid;

#[derive(Deserialize)]
struct ShellCommandArgs {
    cmd: String,
    #[serde(default, deserialize_with = "deserialize_boolish")]
    require_escalated: bool,
    #[serde(default = "shell_default_timeout")]
    timeout_ms: u64,
}

fn shell_default_timeout() -> u64 {
    30_000
}

fn deserialize_boolish<'de, D>(deserializer: D) -> std::result::Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    if let Some(v) = value.as_bool() {
        return Ok(v);
    }
    if let Some(v) = value.as_str() {
        return Ok(matches!(
            v.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes"
        ));
    }
    Ok(false)
}

pub struct ShellCommandHandler;

#[async_trait]
impl ToolHandler for ShellCommandHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let arguments = match &invocation.payload {
            ToolPayload::Function { arguments } => arguments.clone(),
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "shell handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: ShellCommandArgs = serde_json::from_str(&arguments).map_err(|e| {
            FunctionCallError::Internal(format!("failed to parse arguments: {}", e))
        })?;

        let command = args.cmd;
        let timeout_ms = args.timeout_ms;
        let _require_escalated = args.require_escalated;

        let sandbox_policy = match invocation.turn.sandbox_policy {
            crate::tools::SandboxPolicy::None => SandboxMode::DangerFullAccess,
            crate::tools::SandboxPolicy::ReadOnly => SandboxMode::WorkspaceWrite,
            crate::tools::SandboxPolicy::Write => SandboxMode::WorkspaceWrite,
            crate::tools::SandboxPolicy::DangerFullAccess => SandboxMode::DangerFullAccess,
        };

        info!(
            command = %command,
            sandbox_mode = sandbox_policy.as_str(),
            "shell started"
        );

        let policy_outcome = evaluate_tool_invocation_policy(&invocation, true);
        if matches!(policy_outcome.trace.decision, PolicyDecision::Deny) {
            return Err(FunctionCallError::Internal(policy_outcome.trace.reason));
        }

        if let SandboxMode::WorkspaceWrite = sandbox_policy {
            ensure_workspace_relative_paths(&command, &invocation.turn.cwd).map_err(|e| {
                FunctionCallError::Internal(format!(
                    "command blocked by workspace_write sandbox policy: {e}"
                ))
            })?;
        }

        let command_tokens: Vec<String> =
            command.split_whitespace().map(|s| s.to_string()).collect();

        if let Some(output) = apply_patch_interceptor::intercept_apply_patch(
            &command_tokens,
            &invocation.turn.cwd,
            None,
            &invocation.turn,
            &invocation.call_id,
            &invocation.tool_name,
        )
        .await?
        {
            return Ok(ToolOutput::Function {
                body: FunctionCallOutputBody::Text(output),
                success: Some(true),
            });
        }

        let shell = Shell::new(ShellType::default())
            .map_err(|e| FunctionCallError::Internal(e.to_string()))?;
        let runtime_settings = SandboxRuntimeSettings::from_env();
        let execution_request = SandboxExecutionRequest {
            command: command.clone(),
            cwd: invocation.turn.cwd.clone(),
            timeout_ms,
            capabilities: policy_outcome.trace.capabilities.clone(),
            shell,
            policy_trace: policy_outcome.trace.clone(),
            preferred_backend: policy_outcome
                .approval_context
                .as_ref()
                .and_then(|ctx| ctx.sandbox_backend),
        };

        let execution = execute_in_sandbox(&execution_request, runtime_settings).await;
        let output = match execution {
            Ok(result) => {
                audit_log(runtime_settings, &execution_request, Some(&result), None);
                result
            }
            Err(backend_error) => {
                audit_log(
                    runtime_settings,
                    &execution_request,
                    None,
                    Some(&backend_error),
                );
                match runtime_settings.degrade_policy {
                    SandboxDegradePolicy::Deny => {
                        return Err(FunctionCallError::Internal(format!(
                            "sandbox backend `{}` unavailable: {}",
                            backend_error.backend.as_str(),
                            backend_error.reason
                        )));
                    }
                    SandboxDegradePolicy::AskThenAllow => {
                        let needs_extra_approval = !matches!(
                            execution_request.policy_trace.decision,
                            PolicyDecision::Allow
                        );
                        if needs_extra_approval
                            && !request_degrade_approval(
                                &invocation,
                                &command,
                                backend_error.backend.as_str(),
                                &backend_error.reason,
                            )
                            .await?
                        {
                            return Err(FunctionCallError::ApprovalRejected(
                                "command rejected by user after sandbox degradation warning"
                                    .to_string(),
                            ));
                        }
                        let (exit_code, stdout, stderr) = run_without_sandbox(&execution_request)
                            .await
                            .map_err(|e| FunctionCallError::Internal(e.to_string()))?;
                        crate::tools::SandboxExecutionResult {
                            exit_code,
                            stdout,
                            stderr,
                            backend_used: crate::tools::SandboxBackend::NoneEscalated,
                            degrade_reason: Some(format!(
                                "{}: {}",
                                backend_error.backend.as_str(),
                                backend_error.reason
                            )),
                            policy_trace: execution_request.policy_trace.clone(),
                        }
                    }
                }
            }
        };

        info!(
            command = %command,
            exit_code = output.exit_code,
            backend = output.backend_used.as_str(),
            stdout_len = output.stdout.len(),
            stderr_len = output.stderr.len(),
            stdout_preview = %summarize_preview(&output.stdout, 240),
            stderr_preview = %summarize_preview(&output.stderr, 240),
            "shell completed"
        );

        let (is_shell_success, result_class) =
            classify_shell_result(output.exit_code, &output.stdout, &output.stderr);

        let model_output = format!(
            "result_class={}\ncommand={}\nexit_code={}\nbackend={}\ndegrade_reason={}\npolicy_decision={:?}\nstdout:\n{}\nstderr:\n{}",
            result_class,
            command,
            output.exit_code,
            output.backend_used.as_str(),
            output.degrade_reason.unwrap_or_else(|| "none".to_string()),
            output.policy_trace.decision,
            output.stdout,
            output.stderr
        );
        info!(
            command = %command,
            result_class = %result_class,
            output_len = model_output.len(),
            output_preview = %summarize_preview(&model_output, 300),
            "shell output prepared for model"
        );

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(model_output),
            success: Some(is_shell_success),
        })
    }
}

fn classify_shell_result(exit_code: i32, stdout: &str, stderr: &str) -> (bool, &'static str) {
    let stderr_trimmed = stderr.trim();
    let stdout_trimmed = stdout.trim();
    if exit_code == 0 {
        if stdout_trimmed.is_empty() && looks_like_fatal_stderr(stderr_trimmed) {
            return (false, "failure");
        }
        return (true, "success");
    }
    if exit_code == 141 && !stdout_trimmed.is_empty() && stderr_trimmed.is_empty() {
        return (true, "partial_success");
    }
    (false, "failure")
}

fn looks_like_fatal_stderr(stderr: &str) -> bool {
    if stderr.is_empty() {
        return false;
    }
    let lower = stderr.to_ascii_lowercase();
    lower.contains("operation not permitted")
        || lower.contains("permission denied")
        || lower.contains("command not found")
        || lower.contains("illegal option")
        || lower.contains("no such file or directory")
}

#[cfg(test)]
mod tests {
    use super::classify_shell_result;

    #[test]
    fn classifies_zero_exit_with_fatal_stderr_as_failure() {
        let (ok, class) = classify_shell_result(0, "", "/bin/sh: /bin/ps: Operation not permitted");
        assert!(!ok);
        assert_eq!(class, "failure");
    }

    #[test]
    fn classifies_sigpipe_with_output_as_partial_success() {
        let (ok, class) = classify_shell_result(141, "some output", "");
        assert!(ok);
        assert_eq!(class, "partial_success");
    }
}

async fn request_degrade_approval(
    invocation: &ToolInvocation,
    command: &str,
    backend: &str,
    reason: &str,
) -> Result<bool, FunctionCallError> {
    let approval_policy = invocation.turn.approval_policy;
    if matches!(approval_policy, ApprovalPolicy::Never) {
        return Err(FunctionCallError::Internal(format!(
            "sandbox backend unavailable and approval policy is never: {backend} {reason}"
        )));
    }

    let request_id = Uuid::new_v4().to_string();
    let human_reason = format!(
        "sandbox backend unavailable; fallback requires explicit approval ({backend}: {reason})"
    );

    if let Some(sink) = &invocation.turn.event_sink {
        let _ = sink.send(Event::ApprovalRequested {
            turn_id: invocation.turn.turn_id,
            request_id: request_id.clone(),
            target: command.to_string(),
            reason: human_reason.clone(),
            tool_name: Some(invocation.tool_name.clone()),
            command_preview: Some(truncate_preview(command, 180)),
            risk_tags: vec!["sandbox_degrade".to_string()],
            sandbox_backend: Some(backend.to_string()),
            degrade_reason: Some(reason.to_string()),
        });
    }

    let approved = invocation
        .turn
        .approval_handler
        .request_approval(ApprovalRequest {
            request_id: request_id.clone(),
            target: command.to_string(),
            reason: human_reason,
        })
        .await
        .map_err(FunctionCallError::Internal)?;

    if let Some(sink) = &invocation.turn.event_sink {
        let _ = sink.send(Event::ApprovalResolved {
            turn_id: invocation.turn.turn_id,
            request_id,
            approved,
        });
    }

    Ok(approved)
}

fn truncate_preview(command: &str, limit: usize) -> String {
    let total = command.chars().count();
    if total <= limit {
        return command.to_string();
    }
    let mut preview = command.chars().take(limit).collect::<String>();
    preview.push_str("...");
    preview
}

fn summarize_preview(text: &str, limit: usize) -> String {
    let normalized = text.replace('\n', "\\n").replace('\r', "\\r");
    let total = normalized.chars().count();
    if total <= limit {
        return normalized;
    }
    let mut preview = normalized.chars().take(limit).collect::<String>();
    preview.push_str("...");
    preview
}
