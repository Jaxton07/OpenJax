use openjax_protocol::Event;

use crate::state::{ApprovalSelection, LiveMessage, PendingApproval};

use super::App;

impl App {
    pub fn apply_core_event(&mut self, event: Event) {
        match event {
            Event::TurnStarted { turn_id } => {
                self.state.active_turn_id = Some(turn_id);
                self.state.stream_turn_id = None;
                self.state.stream_text.clear();
                self.set_live_status("Thinking...");
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
                let suffix = target.unwrap_or_default();
                let cell = self.tool_cell(if suffix.is_empty() {
                    format!("Run {}", tool_name)
                } else {
                    format!("Run {} ({})", tool_name, suffix)
                });
                self.queue_history_cell(cell);
            }
            Event::ToolCallCompleted {
                tool_name,
                ok,
                output,
                ..
            } => {
                let cell = self.tool_completed_cell(&tool_name, ok, &output);
                self.queue_history_cell(cell);
            }
            Event::ApprovalRequested {
                request_id,
                target,
                reason,
                ..
            } => {
                self.state.pending_approval = Some(PendingApproval {
                    request_id,
                    target,
                    reason,
                });
                self.state.approval_selection = ApprovalSelection::Approve;
                if let Some(pending) = &self.state.pending_approval {
                    self.state.live_messages = vec![LiveMessage {
                        role: "approval",
                        content: format!(
                            "{} - {} (input y/n + Enter)",
                            pending.target, pending.reason
                        ),
                    }];
                }
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
                self.state.live_messages.clear();
            }
            Event::ShutdownComplete => {
                self.set_live_status("Shutdown complete");
            }
            Event::AgentSpawned { .. } | Event::AgentStatusChanged { .. } => {}
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
        assert!(app.drain_history_cells().is_empty());
    }
}
