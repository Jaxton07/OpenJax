use anyhow::Result;
use tokio::time::{Duration, timeout};
use uuid::Uuid;

use crate::approval::{ApprovalRequest, approval_timeout_ms_from_env};
use crate::sandbox::policy::extract_shell_risk_tags;
use crate::sandbox::runtime::fnv1a64;
use crate::tools::context::ToolInvocation;
use crate::tools::error::FunctionCallError;
use openjax_policy::schema::{DecisionKind, PolicyInput};
use openjax_protocol::{ApprovalKind, Event};
use std::time::Instant;

/// 查询 Policy Center 是否允许沙箱降级后提权执行。
///
/// 返回 Policy Center 的决策种类：
/// - `Allow`：直接允许，无需审批
/// - `Ask` / `Escalate`：需要审批（Escalate 对应 ApprovalKind::Escalation）
/// - `Deny`：拒绝，不触发审批
fn query_policy_center_for_degrade(invocation: &ToolInvocation, command: &str) -> DecisionKind {
    let Some(runtime) = &invocation.turn.policy_runtime else {
        // 无 policy runtime，回退为 Escalate（需要升级审批）
        return DecisionKind::Escalate;
    };

    let handle = runtime.handle();
    let mut risk_tags = extract_shell_risk_tags(command, false);
    if !risk_tags.contains(&"sandbox_degrade".to_string()) {
        risk_tags.push("sandbox_degrade".to_string());
    }

    let input = PolicyInput {
        tool_name: invocation.tool_name.clone(),
        action: "exec".to_string(),
        session_id: invocation
            .turn
            .session_id
            .clone()
            .or_else(|| Some(invocation.turn.turn_id.to_string())),
        actor: Some("user".to_string()),
        resource: Some(invocation.turn.cwd.display().to_string()),
        capabilities: vec!["process_exec".to_string()],
        risk_tags,
        policy_version: handle.policy_version(),
    };

    handle.decide(&input).kind
}

pub async fn request_degrade_approval(
    invocation: &ToolInvocation,
    command: &str,
    backend: &str,
    reason: &str,
) -> Result<bool, FunctionCallError> {
    let request_id = Uuid::new_v4().to_string();
    let command_hash = fnv1a64(command);
    let human_reason = format!(
        "sandbox backend unavailable; fallback requires explicit approval ({backend}: {reason})"
    );

    let policy_decision = query_policy_center_for_degrade(invocation, command);

    // Policy Center 明确拒绝：不触发审批，直接返回错误
    if policy_decision == DecisionKind::Deny {
        tracing::warn!(
            turn_id = invocation.turn.turn_id,
            tool_name = %invocation.tool_name,
            backend = %backend,
            command_hash = %command_hash,
            "degrade_approval_denied_by_policy"
        );
        return Err(FunctionCallError::Internal(format!(
            "policy denied sandbox degrade for command (hash: {command_hash})"
        )));
    }

    let approval_kind = match policy_decision {
        DecisionKind::Escalate => Some(ApprovalKind::Escalation),
        _ => Some(ApprovalKind::Normal),
    };

    tracing::info!(
        turn_id = invocation.turn.turn_id,
        request_id = %request_id,
        tool_name = %invocation.tool_name,
        backend = %backend,
        degrade_reason = %reason,
        command_hash = %command_hash,
        approval_kind = ?approval_kind,
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
            approval_kind,
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
