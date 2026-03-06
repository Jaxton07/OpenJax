use crate::history_cell::HistoryCell;
use std::time::Instant;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlashMatch {
    pub command_name: &'static str,
    pub description: &'static str,
    pub usage_hint: &'static str,
    pub replacement: String,
    pub kind: SlashCommandKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlashCommandKind {
    LocalAction(SlashLocalAction),
    PromptTemplate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlashLocalAction {
    Clear,
    Help,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct SlashPaletteState {
    pub visible: bool,
    pub query: String,
    pub matches: Vec<SlashMatch>,
    pub selected_index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveMessage {
    pub role: &'static str,
    pub content: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusPhase {
    Running,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusBarState {
    pub phase: StatusPhase,
    pub label: String,
    pub show_interrupt_hint: bool,
    pub started_at: Instant,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingApproval {
    pub request_id: String,
    pub target: String,
    pub reason: String,
    pub tool_name: Option<String>,
    pub command_preview: Option<String>,
    pub risk_tags: Vec<String>,
    pub sandbox_backend: Option<String>,
    pub degrade_reason: Option<String>,
    pub requested_at: Instant,
    pub timeout_ms: u64,
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
    pub input_cursor: usize,
    pub input_history: Vec<String>,
    pub history_nav_index: Option<usize>,
    pub history_nav_draft: String,
    pub history_cells: Vec<HistoryCell>,
    pub pending_history_cells: Vec<HistoryCell>,
    pub live_messages: Vec<LiveMessage>,
    pub status_bar: Option<StatusBarState>,
    pub slash_palette: SlashPaletteState,
    pub pending_approval: Option<PendingApproval>,
    pub approval_selection: ApprovalSelection,
    pub active_turn_id: Option<u64>,
    pub stream_turn_id: Option<u64>,
    pub stream_text: String,
    pub last_assistant_committed_turn: Option<u64>,
    pub model_name: Option<String>,
    pub approval_policy: Option<String>,
    pub sandbox_mode: Option<String>,
    pub cwd_display: Option<String>,
    pub next_cell_id: u64,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            banner_printed: false,
            input: String::new(),
            input_cursor: 0,
            input_history: Vec::new(),
            history_nav_index: None,
            history_nav_draft: String::new(),
            history_cells: Vec::new(),
            pending_history_cells: Vec::new(),
            live_messages: Vec::new(),
            status_bar: None,
            slash_palette: SlashPaletteState::default(),
            pending_approval: None,
            approval_selection: ApprovalSelection::Approve,
            active_turn_id: None,
            stream_turn_id: None,
            stream_text: String::new(),
            last_assistant_committed_turn: None,
            model_name: None,
            approval_policy: None,
            sandbox_mode: None,
            cwd_display: None,
            next_cell_id: 1,
        }
    }
}
