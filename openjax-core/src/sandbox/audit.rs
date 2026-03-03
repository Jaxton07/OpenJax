use tracing::info;

use crate::sandbox::runtime::{
    BackendUnavailable, SandboxExecutionRequest, SandboxExecutionResult, SandboxRuntimeSettings,
    fnv1a64, summarize_capabilities,
};

pub fn audit_log(
    settings: SandboxRuntimeSettings,
    request: &SandboxExecutionRequest,
    result: Option<&SandboxExecutionResult>,
    backend_error: Option<&BackendUnavailable>,
) {
    if !settings.audit_enabled {
        return;
    }
    let command_hash = fnv1a64(&request.command);
    let caps = summarize_capabilities(&request.capabilities);
    if let Some(ok) = result {
        info!(
            command_hash = %command_hash,
            backend = ok.backend_used.as_str(),
            capabilities = %caps,
            decision = ?ok.policy_trace.decision,
            degrade_reason = ?ok.degrade_reason,
            "sandbox audit success"
        );
        return;
    }
    if let Some(err) = backend_error {
        info!(
            command_hash = %command_hash,
            backend = err.backend.as_str(),
            capabilities = %caps,
            reason = %err.reason,
            "sandbox audit backend unavailable"
        );
    }
}
