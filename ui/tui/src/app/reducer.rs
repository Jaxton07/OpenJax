use openjax_protocol::Event;
use std::time::Instant;
use tracing::info;

use crate::state::{ApprovalSelection, LiveMessage, PendingApproval};
use openjax_core::approval_timeout_ms_from_env;

use super::App;
use super::tool_output::sanitize_target_for_title;

impl App {
    pub fn apply_core_event(&mut self, event: Event) {
        match event {
            Event::TurnStarted { turn_id } => {
                self.state.active_turn_id = Some(turn_id);
                self.state.stream_turn_id = None;
                self.state.stream_text.clear();
                self.set_status_running("Working");
            }
            Event::AssistantDelta {
                turn_id,
                content_delta,
            } => {
                if self.state.stream_turn_id != Some(turn_id) {
                    self.state.stream_turn_id = Some(turn_id);
                    self.state.stream_text.clear();
                }
                self.state.stream_text.push_str(&content_delta);
                self.state.live_messages = vec![LiveMessage {
                    role: "assistant",
                    content: self.state.stream_text.clone(),
                }];
            }
            Event::AssistantMessage { turn_id, content } => {
                self.state.stream_turn_id = Some(turn_id);
                self.state.stream_text = content.clone();
                let cell = self.assistant_cell(&content);
                self.queue_history_cell(cell);
                self.state.last_assistant_committed_turn = Some(turn_id);
                self.state.live_messages.clear();
            }
            Event::ToolCallStarted {
                tool_name, target, ..
            } => {
                let suffix = target
                    .as_deref()
                    .map(|raw| sanitize_target_for_title(raw, 120))
                    .unwrap_or_default();
                let cell = self.tool_cell(if suffix.is_empty() {
                    format!("Run {}", tool_name)
                } else {
                    format!("Run {} ({})", tool_name, suffix)
                });
                self.queue_history_cell(cell);
                info!(
                    tool_name = %tool_name,
                    pending_cells = self.state.pending_history_cells.len(),
                    "tui applied ToolCallStarted"
                );
            }
            Event::ToolCallCompleted {
                tool_name,
                ok,
                output,
                ..
            } => {
                let cell = self.tool_completed_cell(&tool_name, ok, &output);
                self.queue_history_cell(cell);
                info!(
                    tool_name = %tool_name,
                    ok = ok,
                    pending_cells = self.state.pending_history_cells.len(),
                    "tui applied ToolCallCompleted"
                );
            }
            Event::ApprovalRequested {
                request_id,
                target,
                reason,
                tool_name,
                command_preview,
                risk_tags,
                sandbox_backend,
                degrade_reason,
                ..
            } => {
                let now = Instant::now();
                let timeout_ms = approval_timeout_ms_from_env();
                let mut dedup_request_id: Option<String> = None;
                if let Some(existing) = self.state.pending_approval.as_mut() {
                    if existing.request_id == request_id {
                        // Same request emitted from multiple channels: merge richer fields
                        // without creating a second approval state transition.
                        existing.target = target.clone();
                        existing.reason = reason.clone();
                        if existing.tool_name.is_none() {
                            existing.tool_name = tool_name.clone();
                        }
                        if existing.command_preview.is_none() {
                            existing.command_preview = command_preview.clone();
                        }
                        if existing.risk_tags.is_empty() {
                            existing.risk_tags = risk_tags.clone();
                        }
                        if existing.sandbox_backend.is_none() {
                            existing.sandbox_backend = sandbox_backend.clone();
                        }
                        if existing.degrade_reason.is_none() {
                            existing.degrade_reason = degrade_reason.clone();
                        }
                        existing.requested_at = now;
                        existing.timeout_ms = timeout_ms;
                        self.state.approval_selection = ApprovalSelection::Approve;
                        dedup_request_id = Some(existing.request_id.clone());
                    }
                }
                if let Some(request_id) = dedup_request_id {
                    self.refresh_approval_live_message();
                    info!(request_id = %request_id, "tui dedup ApprovalRequested");
                    return;
                }

                self.state.pending_approval = Some(PendingApproval {
                    request_id,
                    target,
                    reason,
                    tool_name,
                    command_preview,
                    risk_tags,
                    sandbox_backend,
                    degrade_reason,
                    requested_at: now,
                    timeout_ms,
                });
                self.state.approval_selection = ApprovalSelection::Approve;
                self.refresh_approval_live_message();
                info!(
                    request_id = %self
                        .state
                        .pending_approval
                        .as_ref()
                        .map(|p| p.request_id.as_str())
                        .unwrap_or(""),
                    "tui applied ApprovalRequested"
                );
            }
            Event::ApprovalResolved {
                request_id,
                approved,
                ..
            } => {
                self.state.pending_approval = None;
                self.state.live_messages.clear();
                let cell = self.system_cell(format!(
                    "approval resolved {} ({})",
                    if approved { "approved" } else { "rejected" },
                    request_id
                ));
                self.queue_history_cell(cell);
                info!(
                    request_id = %request_id,
                    approved = approved,
                    pending_cells = self.state.pending_history_cells.len(),
                    "tui applied ApprovalResolved"
                );
            }
            Event::TurnCompleted { turn_id } => {
                self.state.active_turn_id = None;
                if self.state.stream_turn_id == Some(turn_id)
                    && !self.state.stream_text.is_empty()
                    && self.state.last_assistant_committed_turn != Some(turn_id)
                {
                    let content = self.state.stream_text.clone();
                    let cell = self.assistant_cell(&content);
                    self.queue_history_cell(cell);
                    self.state.last_assistant_committed_turn = Some(turn_id);
                }
                self.state.stream_text.clear();
                self.clear_status_bar();
                self.state.live_messages.clear();
            }
            Event::ShutdownComplete => {
                self.clear_status_bar();
                self.set_live_status("Shutdown complete");
            }
            Event::AgentSpawned { .. } | Event::AgentStatusChanged { .. } => {}
        }
    }

    fn refresh_approval_live_message(&mut self) {
        if let Some(pending) = &self.state.pending_approval {
            let content = format!("pending ({}) (input y/n + Enter)", pending.request_id);
            self.state.live_messages = vec![LiveMessage {
                role: "approval",
                content,
            }];
        }
    }
}

#[cfg(test)]
mod tests {
    use openjax_protocol::Event;

    use super::App;

    #[test]
    fn stream_commit_boundary_is_stable() {
        let mut app = App::default();

        app.apply_core_event(Event::TurnStarted { turn_id: 1 });
        assert!(app.state.status_bar.is_some());
        app.apply_core_event(Event::AssistantDelta {
            turn_id: 1,
            content_delta: "hello".to_string(),
        });
        assert!(app.drain_history_cells().is_empty());

        app.apply_core_event(Event::AssistantMessage {
            turn_id: 1,
            content: "hello".to_string(),
        });
        assert_eq!(app.drain_history_cells().len(), 1);

        app.apply_core_event(Event::TurnCompleted { turn_id: 1 });
        assert!(app.state.status_bar.is_none());
        assert!(app.drain_history_cells().is_empty());
    }
}
