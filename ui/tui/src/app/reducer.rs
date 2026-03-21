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
            Event::ResponseStarted { turn_id, .. } | Event::ResponseResumed { turn_id, .. } => {
                self.state.active_turn_id = Some(turn_id);
                if self.state.stream_turn_id != Some(turn_id) {
                    self.state.stream_turn_id = Some(turn_id);
                    self.state.stream_text.clear();
                }
                self.set_status_running("Working");
            }
            Event::ResponseTextDelta {
                turn_id,
                content_delta,
                ..
            } => {
                self.apply_stream_delta(turn_id, &content_delta);
            }
            Event::ResponseCompleted {
                turn_id, content, ..
            } => {
                self.state.stream_turn_id = Some(turn_id);
                self.state.stream_text = content;
                self.state.live_messages.clear();
            }
            Event::AssistantMessage { turn_id, content } => {
                self.state.stream_turn_id = Some(turn_id);
                self.state.stream_text = content.clone();
                if self.state.last_assistant_committed_turn != Some(turn_id) {
                    let cell = self.assistant_cell(&content);
                    self.queue_history_cell(cell);
                    self.state.last_assistant_committed_turn = Some(turn_id);
                }
                self.state.live_messages.clear();
            }
            Event::ToolCallStarted {
                tool_name,
                target,
                display_name,
                ..
            } => {
                let display = display_name.as_ref().unwrap_or(&tool_name);
                let suffix = target
                    .as_deref()
                    .map(|raw| sanitize_target_for_title(raw, 120))
                    .unwrap_or_default();
                let cell = self.tool_cell(if suffix.is_empty() {
                    format!("Run {}", display)
                } else {
                    format!("Run {} ({})", display, suffix)
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
            Event::ToolCallArgsDelta { .. }
            | Event::ToolCallReady { .. }
            | Event::ToolCallProgress { .. } => {}
            Event::ToolCallFailed {
                tool_name, message, ..
            } => {
                let cell = self.system_cell(format!("tool {} failed: {}", tool_name, message));
                self.queue_history_cell(cell);
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
                if let Some(existing) = self.state.pending_approval.as_mut()
                    && existing.request_id == request_id
                {
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
                self.dismiss_slash_palette();
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
                if self
                    .state
                    .pending_approval
                    .as_ref()
                    .is_some_and(|pending| pending.request_id != request_id)
                {
                    info!(
                        request_id = %request_id,
                        pending_request_id = %self
                            .state
                            .pending_approval
                            .as_ref()
                            .map(|pending| pending.request_id.as_str())
                            .unwrap_or(""),
                        "ignoring stale ApprovalResolved for non-current request"
                    );
                    return;
                }
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
                // Allow the viewport to shrink back after the turn finishes so that
                // the expanded live-zone (grown during streaming) collapses to the
                // minimum height instead of leaving a block of blank lines.
                self.viewport_reset_requested = true;
            }
            Event::ShutdownComplete => {
                self.clear_status_bar();
                self.set_live_status("Shutdown complete");
            }
            Event::ToolCallsProposed { .. } | Event::ToolBatchCompleted { .. } => {}
            Event::ResponseError { message, .. } => {
                self.clear_status_bar();
                self.set_live_status(format!("Response failed: {message}"));
            }
            Event::ReasoningDelta { .. } => {}
            Event::LoopWarning { .. } => {}
            Event::AgentSpawned { .. } | Event::AgentStatusChanged { .. } => {}
            Event::ContextUsageUpdated { .. } => {}
            // TODO: display context compaction notification in status bar
            Event::ContextCompacted { .. } => {}
        }
    }

    fn apply_stream_delta(&mut self, turn_id: u64, content_delta: &str) {
        if self.state.stream_turn_id != Some(turn_id) {
            self.state.stream_turn_id = Some(turn_id);
            self.state.stream_text.clear();
        }
        self.state.stream_text.push_str(content_delta);
        self.state.live_messages = vec![LiveMessage {
            role: "assistant",
            content: self.state.stream_text.clone(),
        }];
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
    use std::time::Instant;

    use openjax_protocol::{Event, StreamSource};

    use super::App;
    use crate::state::{LiveMessage, PendingApproval};

    #[test]
    fn stream_commit_boundary_is_stable() {
        let mut app = App::default();

        app.apply_core_event(Event::TurnStarted { turn_id: 1 });
        assert!(app.state.status_bar.is_some());
        app.apply_core_event(Event::ResponseTextDelta {
            turn_id: 1,
            content_delta: "hello".to_string(),
            stream_source: StreamSource::ModelLive,
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

    #[test]
    fn turn_completed_requests_viewport_reset_to_collapse_blank_lines() {
        // Regression: after streaming, the live-zone expanded the viewport height.
        // TurnCompleted must signal a viewport reset so the sticky height is released
        // and the blank-line gap below the history content is eliminated.
        let mut app = App::default();

        app.apply_core_event(Event::TurnStarted { turn_id: 1 });
        app.apply_core_event(Event::ResponseTextDelta {
            turn_id: 1,
            content_delta: "some streamed text".to_string(),
            stream_source: StreamSource::ModelLive,
        });
        // Not yet requested before turn completes.
        assert!(!app.take_viewport_reset_requested());

        app.apply_core_event(Event::TurnCompleted { turn_id: 1 });
        // Must be set so tui.rs can allow the viewport to shrink.
        assert!(app.take_viewport_reset_requested());
        // Consumed; second call returns false.
        assert!(!app.take_viewport_reset_requested());
    }

    #[test]
    fn assistant_message_duplicate_turn_is_idempotent() {
        let mut app = App::default();

        app.apply_core_event(Event::AssistantMessage {
            turn_id: 7,
            content: "hello".to_string(),
        });
        assert_eq!(app.drain_history_cells().len(), 1);

        app.apply_core_event(Event::AssistantMessage {
            turn_id: 7,
            content: "hello".to_string(),
        });
        assert!(
            app.drain_history_cells().is_empty(),
            "duplicate assistant event for same turn should not enqueue a second history cell",
        );
    }

    #[test]
    fn stale_approval_resolved_does_not_clear_current_pending() {
        let mut app = App::default();
        app.state.pending_approval = Some(PendingApproval {
            request_id: "req-current".to_string(),
            target: "target".to_string(),
            reason: "reason".to_string(),
            tool_name: Some("shell".to_string()),
            command_preview: Some("echo hi".to_string()),
            risk_tags: vec!["write".to_string()],
            sandbox_backend: Some("linux_native".to_string()),
            degrade_reason: None,
            requested_at: Instant::now(),
            timeout_ms: 30_000,
        });
        app.state.live_messages = vec![LiveMessage {
            role: "approval",
            content: "pending (req-current)".to_string(),
        }];

        app.apply_core_event(Event::ApprovalResolved {
            turn_id: 1,
            request_id: "req-stale".to_string(),
            approved: true,
        });

        assert_eq!(
            app.state
                .pending_approval
                .as_ref()
                .map(|pending| pending.request_id.as_str()),
            Some("req-current"),
        );
        assert_eq!(app.state.live_messages.len(), 1);
        assert!(app.drain_history_cells().is_empty());
    }
}
