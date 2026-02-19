use crate::render::markdown::render_markdown_as_plain_text;
use crate::ui::overlay_approval::ApprovalOverlay;
use openjax_protocol::Event;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppState {
    pub messages: Vec<UiMessage>,
    pub input: String,
    pub approval_overlay: Option<ApprovalOverlay>,
    pub show_help: bool,
}

impl AppState {
    pub fn push_user_message(&mut self, content: String) {
        self.messages.push(UiMessage {
            role: "user".to_string(),
            content,
        });
    }

    pub fn push_system_message(&mut self, content: String) {
        self.messages.push(UiMessage {
            role: "system".to_string(),
            content,
        });
    }

    pub fn push_assistant_message(&mut self, content: String) {
        self.messages.push(UiMessage {
            role: "assistant".to_string(),
            content,
        });
    }

    pub fn map_core_event(&mut self, event: &Event) {
        match event {
            Event::AssistantMessage { content, .. } => {
                self.push_assistant_message(render_markdown_as_plain_text(content));
            }
            Event::AssistantDelta { content_delta, .. } => {
                if let Some(last) = self.messages.last_mut()
                    && last.role == "assistant"
                {
                    last.content.push_str(content_delta);
                } else {
                    self.push_assistant_message(content_delta.clone());
                }
            }
            Event::ToolCallStarted { tool_name, .. } => {
                self.push_system_message(format!("tool started: {tool_name}"));
            }
            Event::ToolCallCompleted {
                tool_name,
                ok,
                output,
                ..
            } => {
                self.push_system_message(format!(
                    "tool completed: {tool_name} (ok={ok}) {}",
                    truncate(output, 160)
                ));
            }
            Event::TurnStarted { turn_id } => {
                self.push_system_message(format!("turn started: {turn_id}"));
            }
            Event::TurnCompleted { turn_id } => {
                self.push_system_message(format!("turn completed: {turn_id}"));
            }
            Event::ShutdownComplete => {
                self.push_system_message("shutdown complete".to_string());
            }
            Event::ApprovalRequested {
                request_id,
                target,
                reason,
                ..
            } => {
                self.approval_overlay = Some(ApprovalOverlay::new(
                    request_id.clone(),
                    format!("approve `{target}` ? {reason}"),
                ));
            }
            Event::ApprovalResolved {
                request_id,
                approved,
                ..
            } => {
                if self
                    .approval_overlay
                    .as_ref()
                    .is_some_and(|overlay| overlay.request_id == *request_id)
                {
                    self.approval_overlay = None;
                }
                self.push_system_message(format!(
                    "approval resolved: id={request_id} approved={approved}"
                ));
            }
            Event::AgentSpawned { .. } | Event::AgentStatusChanged { .. } => {}
        }
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut out = String::new();
    for ch in s.chars().take(max_chars) {
        out.push(ch);
    }
    out.push_str("...");
    out
}
