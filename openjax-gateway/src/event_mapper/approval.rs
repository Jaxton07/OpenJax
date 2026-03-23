use openjax_protocol::Event;
use serde_json::json;

use super::CoreEventMapping;

pub fn map(event: &Event) -> Option<CoreEventMapping> {
    match event {
        Event::ApprovalRequested {
            turn_id,
            request_id: approval_id,
            target,
            reason,
            policy_version,
            matched_rule_id,
            tool_name,
            command_preview,
            risk_tags,
            sandbox_backend,
            degrade_reason,
        } => Some(CoreEventMapping {
            core_turn_id: Some(*turn_id),
            event_type: "approval_requested",
            payload: json!({
                "approval_id": approval_id,
                "target": target,
                "reason": reason,
                "policy_version": policy_version,
                "matched_rule_id": matched_rule_id,
                "tool_name": tool_name,
                "command_preview": command_preview,
                "risk_tags": risk_tags,
                "sandbox_backend": sandbox_backend,
                "degrade_reason": degrade_reason
            }),
            stream_source: None,
        }),
        Event::ApprovalResolved {
            turn_id,
            request_id: approval_id,
            approved,
        } => Some(CoreEventMapping {
            core_turn_id: Some(*turn_id),
            event_type: "approval_resolved",
            payload: json!({
                "approval_id": approval_id,
                "approved": approved
            }),
            stream_source: None,
        }),
        _ => None,
    }
}
