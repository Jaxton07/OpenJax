pub mod audit;
pub mod classifier;
pub mod degrade;
pub mod policy;
pub mod result;
pub mod runtime;
pub mod types;

use crate::tools::context::{FunctionCallOutputBody, ToolInvocation, ToolOutput};
use crate::tools::error::FunctionCallError;
use crate::tools::shell::{Shell, ShellType};
use crate::tools::{SandboxMode, SandboxPolicy};
use tracing::info;

use self::classifier::classify_command;
use self::degrade::request_degrade_approval;
use self::policy::{PolicyDecision, SandboxBackend, evaluate_tool_invocation_policy};
use self::result::{classify_shell_result, looks_like_fatal_stderr};
use self::runtime::{
    SandboxDegradePolicy, SandboxExecutionRequest, SandboxRuntimeSettings, execute_in_sandbox,
    run_without_sandbox,
};
use self::types::CommandClass;

pub async fn execute_shell(
    invocation: &ToolInvocation,
    command: &str,
    timeout_ms: u64,
) -> Result<ToolOutput, FunctionCallError> {
    let sandbox_policy = match invocation.turn.sandbox_policy {
        SandboxPolicy::None => SandboxMode::DangerFullAccess,
        SandboxPolicy::ReadOnly => SandboxMode::WorkspaceWrite,
        SandboxPolicy::Write => SandboxMode::WorkspaceWrite,
        SandboxPolicy::DangerFullAccess => SandboxMode::DangerFullAccess,
    };

    info!(
        command = %command,
        sandbox_mode = sandbox_policy.as_str(),
        "shell started"
    );

    let policy_outcome = evaluate_tool_invocation_policy(invocation, true);
    if matches!(policy_outcome.trace.decision, PolicyDecision::Deny) {
        return Err(FunctionCallError::Internal(policy_outcome.trace.reason));
    }

    if let SandboxMode::WorkspaceWrite = sandbox_policy {
        runtime::ensure_workspace_relative_paths(command, &invocation.turn.cwd).map_err(|e| {
            FunctionCallError::Internal(format!(
                "command blocked by workspace_write sandbox policy: {e}"
            ))
        })?;
    }

    let shell =
        Shell::new(ShellType::default()).map_err(|e| FunctionCallError::Internal(e.to_string()))?;
    let runtime_settings = SandboxRuntimeSettings::from_env();
    let execution_request = SandboxExecutionRequest {
        command: command.to_string(),
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

    let command_class = classify_command(command);
    let execution = execute_in_sandbox(&execution_request, runtime_settings).await;
    let mut output = match execution {
        Ok(result) => {
            audit::audit_log(runtime_settings, &execution_request, Some(&result), None);
            result
        }
        Err(backend_error) => {
            audit::audit_log(
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
                    let needs_extra_approval =
                        !matches!(
                            execution_request.policy_trace.decision,
                            PolicyDecision::Allow
                        ) || matches!(command_class, CommandClass::ProcessObserve);
                    if needs_extra_approval
                        && !request_degrade_approval(
                            invocation,
                            command,
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
                    runtime::SandboxExecutionResult {
                        exit_code,
                        stdout,
                        stderr,
                        backend_used: SandboxBackend::NoneEscalated,
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

    let mut runtime_allowed =
        !(output.exit_code == 0 && looks_like_fatal_stderr(output.stderr.trim()));
    let mut runtime_deny_reason = if runtime_allowed {
        None
    } else {
        Some(output.stderr.trim().to_string())
    };

    if !runtime_allowed
        && matches!(command_class, CommandClass::ProcessObserve)
        && !matches!(output.backend_used, SandboxBackend::NoneEscalated)
    {
        match runtime_settings.degrade_policy {
            SandboxDegradePolicy::Deny => {
                return Err(FunctionCallError::Internal(format!(
                    "sandbox runtime denied command under `{}` and degrade policy is deny: {}",
                    output.backend_used.as_str(),
                    output.stderr.trim()
                )));
            }
            SandboxDegradePolicy::AskThenAllow => {
                let runtime_reason = output.stderr.trim().to_string();
                let degrade_reason = format!(
                    "{} runtime denied: {}",
                    output.backend_used.as_str(),
                    runtime_reason
                );
                if !request_degrade_approval(
                    invocation,
                    command,
                    output.backend_used.as_str(),
                    &degrade_reason,
                )
                .await?
                {
                    return Err(FunctionCallError::ApprovalRejected(
                        "command rejected by user after sandbox runtime denial".to_string(),
                    ));
                }
                let (exit_code, stdout, stderr) = run_without_sandbox(&execution_request)
                    .await
                    .map_err(|e| FunctionCallError::Internal(e.to_string()))?;
                output = runtime::SandboxExecutionResult {
                    exit_code,
                    stdout,
                    stderr,
                    backend_used: SandboxBackend::NoneEscalated,
                    degrade_reason: Some(degrade_reason),
                    policy_trace: execution_request.policy_trace.clone(),
                };
                runtime_allowed = true;
                runtime_deny_reason = None;
            }
        }
    }
    let result_class = classify_shell_result(output.exit_code, &output.stdout, &output.stderr);
    let is_shell_success = !matches!(result_class, types::SandboxResultClass::Failure);

    info!(
        command = %command,
        exit_code = output.exit_code,
        backend = output.backend_used.as_str(),
        stdout_len = output.stdout.len(),
        stderr_len = output.stderr.len(),
        runtime_allowed = runtime_allowed,
        "shell completed"
    );

    let model_output = format!(
        "result_class={}\ncommand={}\nexit_code={}\nbackend={}\ndegrade_reason={}\npolicy_decision={:?}\nruntime_allowed={}\nruntime_deny_reason={}\nstdout:\n{}\nstderr:\n{}",
        result_class.as_str(),
        command,
        output.exit_code,
        output.backend_used.as_str(),
        output.degrade_reason.unwrap_or_else(|| "none".to_string()),
        output.policy_trace.decision,
        runtime_allowed,
        runtime_deny_reason.unwrap_or_else(|| "none".to_string()),
        output.stdout,
        output.stderr
    );
    info!(
        command = %command,
        result_class = result_class.as_str(),
        output_len = model_output.len(),
        "shell output prepared for model"
    );

    Ok(ToolOutput::Function {
        body: FunctionCallOutputBody::Text(model_output),
        success: Some(is_shell_success),
    })
}
