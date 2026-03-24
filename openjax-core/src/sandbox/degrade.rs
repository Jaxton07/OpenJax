use anyhow::Result;
use tokio::time::{Duration, timeout};
use uuid::Uuid;

use crate::approval::{ApprovalRequest, approval_timeout_ms_from_env};
use crate::sandbox::runtime::fnv1a64;
use crate::tools::ApprovalPolicy;
use crate::tools::context::ToolInvocation;
use crate::tools::error::FunctionCallError;
use openjax_protocol::Event;
use std::time::Instant;

pub async fn request_degrade_approval(
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
    let command_hash = fnv1a64(command);
    let human_reason = format!(
        "sandbox backend unavailable; fallback requires explicit approval ({backend}: {reason})"
    );
    tracing::info!(
        turn_id = invocation.turn.turn_id,
        request_id = %request_id,
        tool_name = %invocation.tool_name,
        backend = %backend,
        degrade_reason = %reason,
        command_hash = %command_hash,
        "degrade_approval_request_logged"
    );

    if let Some(sink) = &invocation.turn.event_sink {
        let _ = sink.send(Event::ApprovalRequested {
            turn_id: invocation.turn.turn_id,
            request_id: request_id.clone(),
            target: command.to_string(),
            reason: human_reason.clone(),
            policy_version: None,
            matched_rule_id: None,
            tool_name: Some(invocation.tool_name.clone()),
            command_preview: Some(truncate_preview(command, 180)),
            risk_tags: vec!["sandbox_degrade".to_string()],
            sandbox_backend: Some(backend.to_string()),
            degrade_reason: Some(reason.to_string()),
            approval_kind: None,
        });
    }

    let turn = &invocation.turn;
    let timeout_ms = approval_timeout_ms_from_env();
    let approval_start = Instant::now();
    let request = ApprovalRequest {
        request_id: request_id.clone(),
        target: command.to_string(),
        reason: human_reason,
    };
    let approved = match timeout(
        Duration::from_millis(timeout_ms),
        turn.approval_handler.request_approval(request),
    )
    .await
    {
        Ok(result) => result.map_err(FunctionCallError::Internal)?,
        Err(_) => {
            if let Some(sink) = &invocation.turn.event_sink {
                let _ = sink.send(Event::ApprovalResolved {
                    turn_id: invocation.turn.turn_id,
                    request_id: request_id.clone(),
                    approved: false,
                });
            }
            tracing::info!(
                turn_id = invocation.turn.turn_id,
                request_id = %request_id,
                tool_name = %invocation.tool_name,
                backend = %backend,
                approved = false,
                timed_out = true,
                latency_ms = approval_start.elapsed().as_millis() as u64,
                command_hash = %command_hash,
                "degrade_approval_result_logged"
            );
            return Err(FunctionCallError::ApprovalTimedOut(format!(
                "approval request timed out after {}ms ({request_id})",
                timeout_ms
            )));
        }
    };

    if let Some(sink) = &invocation.turn.event_sink {
        let _ = sink.send(Event::ApprovalResolved {
            turn_id: invocation.turn.turn_id,
            request_id: request_id.clone(),
            approved,
        });
    }
    tracing::info!(
        turn_id = invocation.turn.turn_id,
        request_id = %request_id,
        tool_name = %invocation.tool_name,
        backend = %backend,
        approved = approved,
        timed_out = false,
        latency_ms = approval_start.elapsed().as_millis() as u64,
        command_hash = %command_hash,
        "degrade_approval_result_logged"
    );

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
