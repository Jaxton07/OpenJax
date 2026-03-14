use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamSourceKind {
    ModelLive,
    Synthetic,
    Replay,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallLifecycle {
    Started,
    ArgsDelta,
    Progress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamApprovalDecision {
    Approved,
    Rejected,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StreamEventKind {
    TurnStarted,
    TurnCompleted,
    ResponseStarted {
        source: StreamSourceKind,
    },
    ResponseTextDelta {
        source: StreamSourceKind,
        delta: String,
    },
    ResponseCompleted {
        source: StreamSourceKind,
        content: String,
    },
    ResponseError {
        code: String,
        message: String,
        retryable: bool,
    },
    ToolCall {
        tool_call_id: String,
        tool_name: String,
        phase: ToolCallLifecycle,
        payload: String,
    },
    ApprovalRequested {
        approval_id: String,
        target: String,
        reason: String,
    },
    ApprovalResolved {
        approval_id: String,
        decision: StreamApprovalDecision,
    },
    Usage {
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        total_tokens: Option<u64>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamEvent {
    pub turn_id: u64,
    pub event_seq: u64,
    pub turn_seq: u64,
    pub event: StreamEventKind,
}
