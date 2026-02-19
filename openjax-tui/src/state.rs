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
    pub chat_scroll: usize,
    pub follow_output: bool,
    pub input: String,
    pub input_cursor: usize,
    pub input_history: Vec<String>,
    pub input_history_index: Option<usize>,
    pub input_draft: String,
    pub approval_overlay: Option<ApprovalOverlay>,
    pub show_help: bool,
    pub show_system_messages: bool,
    pub model_name: Option<String>,
    pub approval_policy: Option<String>,
    pub sandbox_mode: Option<String>,
}

impl AppState {
    pub fn push_user_message(&mut self, content: String) {
        self.messages.push(UiMessage {
            role: "user".to_string(),
            content,
        });
    }

    pub fn push_system_message(&mut self, content: String) {
        if self.show_system_messages {
            self.messages.push(UiMessage {
                role: "system".to_string(),
                content,
            });
        }
    }

    pub fn push_assistant_message(&mut self, content: String) {
        self.messages.push(UiMessage {
            role: "assistant".to_string(),
            content,
        });
    }

    pub fn insert_input_char(&mut self, ch: char) {
        let mut chars: Vec<char> = self.input.chars().collect();
        let cursor = self.input_cursor.min(chars.len());
        chars.insert(cursor, ch);
        self.input = chars.into_iter().collect();
        self.input_cursor = cursor + 1;
        self.input_history_index = None;
        self.input_draft.clear();
    }

    pub fn backspace_input(&mut self) {
        if self.input_cursor == 0 {
            return;
        }
        let mut chars: Vec<char> = self.input.chars().collect();
        let cursor = self.input_cursor.min(chars.len());
        chars.remove(cursor - 1);
        self.input = chars.into_iter().collect();
        self.input_cursor = cursor - 1;
        self.input_history_index = None;
        self.input_draft.clear();
    }

    pub fn move_cursor_left(&mut self) {
        self.input_cursor = self.input_cursor.saturating_sub(1);
    }

    pub fn move_cursor_right(&mut self) {
        let len = self.input.chars().count();
        self.input_cursor = (self.input_cursor + 1).min(len);
    }

    pub fn recall_prev_history(&mut self) {
        if self.input_history.is_empty() {
            return;
        }
        let idx = match self.input_history_index {
            Some(current) => current.saturating_sub(1),
            None => {
                self.input_draft = self.input.clone();
                self.input_history.len() - 1
            }
        };
        self.input_history_index = Some(idx);
        self.input = self.input_history[idx].clone();
        self.input_cursor = self.input.chars().count();
    }

    pub fn recall_next_history(&mut self) {
        let Some(current) = self.input_history_index else {
            return;
        };
        if current + 1 < self.input_history.len() {
            let idx = current + 1;
            self.input_history_index = Some(idx);
            self.input = self.input_history[idx].clone();
            self.input_cursor = self.input.chars().count();
            return;
        }
        self.input_history_index = None;
        self.input = self.input_draft.clone();
        self.input_cursor = self.input.chars().count();
        self.input_draft.clear();
    }

    pub fn consume_submitted_input(&mut self) -> Option<String> {
        let submitted = self.input.trim().to_string();
        if submitted.is_empty() {
            return None;
        }
        if self.input_history.last() != Some(&submitted) {
            self.input_history.push(submitted.clone());
        }
        self.input.clear();
        self.input_cursor = 0;
        self.input_history_index = None;
        self.input_draft.clear();
        Some(submitted)
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
