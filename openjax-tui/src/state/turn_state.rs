use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TurnPhase {
    #[default]
    Idle,
    Thinking,
    Streaming,
    Error,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TurnState {
    pub phase: TurnPhase,
    pub active_turn_id: Option<u64>,
    pub stream_text_by_turn: HashMap<u64, String>,
    pub render_kind_by_turn: HashMap<u64, RenderKind>,
    pub tool_target_hints: HashMap<(u64, String), VecDeque<String>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RenderKind {
    #[default]
    Plain,
    Markdown,
}

impl TurnState {
    pub fn start_turn(&mut self, turn_id: u64) {
        self.active_turn_id = Some(turn_id);
        self.stream_text_by_turn.entry(turn_id).or_default();
        self.phase = TurnPhase::Thinking;
    }

    pub fn append_delta(&mut self, turn_id: u64, delta: &str) -> String {
        let current = self.stream_text_by_turn.entry(turn_id).or_default();
        current.push_str(delta);
        self.active_turn_id = Some(turn_id);
        self.phase = TurnPhase::Streaming;
        current.clone()
    }

    pub fn set_stream_content(&mut self, turn_id: u64, content: String, kind: RenderKind) {
        self.stream_text_by_turn.insert(turn_id, content);
        self.render_kind_by_turn.insert(turn_id, kind);
        self.active_turn_id = Some(turn_id);
        self.phase = TurnPhase::Streaming;
    }

    pub fn finalize_turn(&mut self, turn_id: u64) -> String {
        let final_text = self
            .stream_text_by_turn
            .remove(&turn_id)
            .unwrap_or_default();
        if self.active_turn_id == Some(turn_id) {
            self.active_turn_id = None;
        }
        self.phase = TurnPhase::Idle;
        final_text
    }

    pub fn set_error(&mut self) {
        self.phase = TurnPhase::Error;
    }

    pub fn add_tool_target_hint(&mut self, turn_id: u64, tool_name: &str, target: &str) {
        let key = (turn_id, tool_name.trim().to_ascii_lowercase());
        let queue = self.tool_target_hints.entry(key).or_default();
        queue.push_back(target.to_string());
    }

    pub fn pop_tool_target_hint(&mut self, turn_id: u64, tool_name: &str) -> Option<String> {
        let key = (turn_id, tool_name.trim().to_ascii_lowercase());
        let queue = self.tool_target_hints.get_mut(&key)?;
        let value = queue.pop_front();
        if queue.is_empty() {
            self.tool_target_hints.remove(&key);
        }
        value
    }
}
