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
                self.push_assistant_message(content.clone());
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
