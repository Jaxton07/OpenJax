pub mod audit;
pub mod classifier;
pub mod degrade;
pub mod policy;
pub mod result;
pub mod runtime;
pub mod types;

use crate::tools::context::{FunctionCallOutputBody, ToolInvocation, ToolOutput};
use crate::tools::error::FunctionCallError;
use crate::tools::shell::Shell;
use crate::tools::{SandboxMode, SandboxPolicy};
use openjax_protocol::ShellExecutionMetadata;
use tracing::info;

use self::classifier::classify_command;
use self::degrade::request_degrade_approval;
use self::policy::{PolicyDecision, SandboxBackend, detect_capabilities, preferred_backend};
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

    let shell_capabilities = detect_capabilities(command);

    // 基于命令能力推导沙箱执行时的 policy_trace.decision：
    // 用于降级审批（degrade approval）是否需要触发的判断。
    // - 有写/网络能力 → AskApproval
    // - 只读命令 → Allow
    let shell_trace_decision = if shell_capabilities.iter().any(|cap| {
        matches!(
            cap,
            self::policy::SandboxCapability::FsWrite
                | self::policy::SandboxCapability::EnvWrite
                | self::policy::SandboxCapability::Network
        )
    }) {
        PolicyDecision::AskApproval
    } else {
        PolicyDecision::Allow
    };

    if let SandboxMode::WorkspaceWrite = sandbox_policy {
        runtime::ensure_workspace_relative_paths(command, &invocation.turn.cwd).map_err(|e| {
            FunctionCallError::Internal(format!(
                "command blocked by workspace_write sandbox policy: {e}"
            ))
        })?;
    }

    let shell = Shell::new(invocation.turn.shell_type)
        .map_err(|e| FunctionCallError::Internal(e.to_string()))?;
    let runtime_settings = SandboxRuntimeSettings::from_env();
    let policy_trace = crate::sandbox::policy::PolicyTrace {
        decision: shell_trace_decision,
        reason: "sandbox execution policy".to_string(),
        risk_tags: vec![],
        capabilities: shell_capabilities.clone(),
    };
    let execution_request = SandboxExecutionRequest {
        command: command.to_string(),
        cwd: invocation.turn.cwd.clone(),
        timeout_ms,
        capabilities: shell_capabilities,
        shell,
        policy_trace,
        preferred_backend: preferred_backend(invocation.turn.sandbox_policy),
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
                    let requires_degrade_approval =
                        !matches!(
                            execution_request.policy_trace.decision,
                            PolicyDecision::Allow
                        ) || matches!(command_class, CommandClass::ProcessObserve)
                            || execution_request
                                .policy_trace
                                .capabilities
                                .iter()
                                .any(|cap| {
                                    matches!(
                                        cap,
                                        self::policy::SandboxCapability::FsWrite
                                            | self::policy::SandboxCapability::EnvWrite
                                            | self::policy::SandboxCapability::Network
                                    )
                                });
                    info!(
                        turn_id = invocation.turn.turn_id,
                        tool_name = %invocation.tool_name,
                        backend = backend_error.backend.as_str(),
                        requires_degrade_approval = requires_degrade_approval,
                        policy_decision = ?execution_request.policy_trace.decision,
                        "degrade_fallback_decision_logged"
                    );
                    if requires_degrade_approval
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

    let (runtime_allowed, _) = evaluate_runtime_status(&output);

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
            }
        }
    }
    let (runtime_allowed, runtime_deny_reason) = evaluate_runtime_status(&output);
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

    let policy_decision = format!("{:?}", output.policy_trace.decision);
    let shell_metadata = ShellExecutionMetadata {
        result_class: result_class.as_str().to_string(),
        backend: output.backend_used.as_str().to_string(),
        exit_code: output.exit_code,
        policy_decision: policy_decision.clone(),
        runtime_allowed,
        degrade_reason: output.degrade_reason.clone(),
        runtime_deny_reason: runtime_deny_reason.clone(),
    };

    let model_content = format!(
        "exit_code={}\nstdout:\n{}\nstderr:\n{}",
        shell_metadata.exit_code, output.stdout, output.stderr
    );
    let display_output = format!(
        "result_class={}\ncommand={}\nbackend={}\npolicy_decision={}\nruntime_allowed={}\ndegrade_reason={}\nruntime_deny_reason={}\n{}",
        shell_metadata.result_class,
        command,
        shell_metadata.backend,
        shell_metadata.policy_decision,
        shell_metadata.runtime_allowed,
        shell_metadata.degrade_reason.as_deref().unwrap_or("none"),
        shell_metadata
            .runtime_deny_reason
            .as_deref()
            .unwrap_or("none"),
        model_content
    );
    info!(
        command = %command,
        result_class = result_class.as_str(),
        output_len = display_output.len(),
        "shell output prepared for model"
    );

    Ok(ToolOutput::Function {
        body: FunctionCallOutputBody::Text(display_output),
        success: Some(is_shell_success),
    })
}

fn evaluate_runtime_status(output: &runtime::SandboxExecutionResult) -> (bool, Option<String>) {
    if output.exit_code != 0 {
        let reason = output.stderr.trim();
        let deny_reason = if reason.is_empty() {
            format!("command exited with non-zero status: {}", output.exit_code)
        } else {
            reason.to_string()
        };
        return (false, Some(deny_reason));
    }

    if looks_like_fatal_stderr(output.stderr.trim()) {
        return (false, Some(output.stderr.trim().to_string()));
    }

    (true, None)
}

#[cfg(test)]
mod tests {
    use super::evaluate_runtime_status;
    use crate::sandbox::policy::{PolicyDecision, PolicyTrace, SandboxBackend};
    use crate::sandbox::runtime::SandboxExecutionResult;

    fn allow_trace() -> PolicyTrace {
        PolicyTrace {
            decision: PolicyDecision::Allow,
            reason: "test".to_string(),
            risk_tags: Vec::new(),
            capabilities: Vec::new(),
        }
    }

    #[test]
    fn process_observe_fallback_failure_stays_runtime_denied() {
        let output = SandboxExecutionResult {
            exit_code: 1,
            stdout: String::new(),
            stderr: "/bin/sh: /bin/ps: Operation not permitted".to_string(),
            backend_used: SandboxBackend::NoneEscalated,
            degrade_reason: Some("macos_seatbelt runtime denied".to_string()),
            policy_trace: allow_trace(),
        };

        let (runtime_allowed, runtime_deny_reason) = evaluate_runtime_status(&output);
        assert!(!runtime_allowed);
        assert_eq!(
            runtime_deny_reason,
            Some("/bin/sh: /bin/ps: Operation not permitted".to_string())
        );
    }

    #[test]
    fn zero_exit_with_fatal_stderr_is_runtime_denied() {
        let output = SandboxExecutionResult {
            exit_code: 0,
            stdout: String::new(),
            stderr: "Operation not permitted".to_string(),
            backend_used: SandboxBackend::MacosSeatbelt,
            degrade_reason: None,
            policy_trace: allow_trace(),
        };

        let (runtime_allowed, runtime_deny_reason) = evaluate_runtime_status(&output);
        assert!(!runtime_allowed);
        assert_eq!(
            runtime_deny_reason,
            Some("Operation not permitted".to_string())
        );
    }
}
