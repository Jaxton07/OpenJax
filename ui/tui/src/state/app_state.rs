use crate::history_cell::HistoryCell;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveMessage {
    pub role: &'static str,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingApproval {
    pub request_id: String,
    pub target: String,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApprovalSelection {
    Approve,
    Deny,
    Later,
}

impl ApprovalSelection {
    pub fn from_index(index: usize) -> Self {
        match index % 3 {
            0 => Self::Approve,
            1 => Self::Deny,
            _ => Self::Later,
        }
    }
}

#[derive(Debug)]
pub struct AppState {
    pub banner_printed: bool,
    pub input: String,
    pub history_cells: Vec<HistoryCell>,
    pub pending_history_cells: Vec<HistoryCell>,
    pub live_messages: Vec<LiveMessage>,
    pub pending_approval: Option<PendingApproval>,
    pub approval_selection: ApprovalSelection,
    pub active_turn_id: Option<u64>,
    pub stream_turn_id: Option<u64>,
    pub stream_text: String,
    pub last_assistant_committed_turn: Option<u64>,
    pub model_name: Option<String>,
    pub approval_policy: Option<String>,
    pub sandbox_mode: Option<String>,
    pub next_cell_id: u64,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            banner_printed: false,
            input: String::new(),
            history_cells: Vec::new(),
            pending_history_cells: Vec::new(),
            live_messages: Vec::new(),
            pending_approval: None,
            approval_selection: ApprovalSelection::Approve,
            active_turn_id: None,
            stream_turn_id: None,
            stream_text: String::new(),
            last_assistant_committed_turn: None,
            model_name: None,
            approval_policy: None,
            sandbox_mode: None,
            next_cell_id: 1,
        }
    }
}
