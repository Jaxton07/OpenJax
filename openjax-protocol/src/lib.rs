use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Op {
    UserTurn { input: String },
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Event {
    TurnStarted {
        turn_id: u64,
    },
    ToolCallStarted {
        turn_id: u64,
        tool_name: String,
    },
    ToolCallCompleted {
        turn_id: u64,
        tool_name: String,
        ok: bool,
        output: String,
    },
    AssistantMessage {
        turn_id: u64,
        content: String,
    },
    TurnCompleted {
        turn_id: u64,
    },
    ShutdownComplete,
}
