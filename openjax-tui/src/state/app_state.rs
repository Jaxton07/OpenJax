use std::collections::VecDeque;

use crate::bottom_pane::slash_commands::{SlashCommandEntry, default_commands};
use crate::state::approval_state::{ApprovalRequestUi, ApprovalSelection, ApprovalState};
use crate::state::input_state::ComposerState;
use crate::state::turn_state::{RenderKind, TurnPhase, TurnState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiMessage {
    pub role: String,
    pub content: String,
    pub render_kind: RenderKind,
    pub ok: bool,
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptState {
    pub messages: Vec<UiMessage>,
    /// Visual-row offset in the wrapped chat viewport.
    pub chat_scroll: usize,
    pub follow_output: bool,
}

impl Default for TranscriptState {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            chat_scroll: 0,
            follow_output: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HistoryEmissionState {
    pub emitted_message_count: usize,
    pub emitted_stream_turn_id: Option<u64>,
    pub has_emitted_any: bool,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub transcript: TranscriptState,
    pub history_emission: HistoryEmissionState,
    pub input_state: ComposerState,
    pub approval: ApprovalState,
    pub turn: TurnState,
    pub show_help: bool,
    pub show_system_messages: bool,
    pub model_name: Option<String>,
    pub approval_policy: Option<String>,
    pub sandbox_mode: Option<String>,
    pub last_error: Option<String>,
    pub pending_decisions: VecDeque<(String, bool)>,
    pub command_catalog: Vec<SlashCommandEntry>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            transcript: TranscriptState::default(),
            history_emission: HistoryEmissionState::default(),
            input_state: ComposerState::default(),
            approval: ApprovalState::default(),
            turn: TurnState::default(),
            show_help: false,
            show_system_messages: false,
            model_name: None,
            approval_policy: None,
            sandbox_mode: None,
            last_error: None,
            pending_decisions: VecDeque::new(),
            command_catalog: default_commands(),
        }
    }
}

impl AppState {
    pub fn with_defaults() -> Self {
        Self::default()
    }

    pub fn push_user_message(&mut self, content: String) {
        self.transcript.messages.push(UiMessage {
            role: "user".to_string(),
            content,
            render_kind: RenderKind::Plain,
            ok: true,
            target: None,
        });
    }

    pub fn push_system_message(&mut self, content: String) {
        if !self.show_system_messages {
            return;
        }
        self.transcript.messages.push(UiMessage {
            role: "system".to_string(),
            content,
            render_kind: RenderKind::Plain,
            ok: true,
            target: None,
        });
    }

    pub fn push_assistant_message(&mut self, content: String, render_kind: RenderKind) {
        self.transcript.messages.push(UiMessage {
            role: "assistant".to_string(),
            content,
            render_kind,
            ok: true,
            target: None,
        });
    }

    pub fn push_tool_message(&mut self, label: String, ok: bool, target: Option<String>) {
        self.transcript.messages.push(UiMessage {
            role: "tool".to_string(),
            content: label,
            render_kind: RenderKind::Plain,
            ok,
            target,
        });
    }

    pub fn clear_messages(&mut self) {
        self.transcript.messages.clear();
        self.history_emission = HistoryEmissionState::default();
        self.turn.active_turn_id = None;
        self.turn.stream_text_by_turn.clear();
        self.turn.render_kind_by_turn.clear();
        self.turn.tool_target_hints.clear();
        self.turn.phase = TurnPhase::Idle;
    }

    pub fn sync_slash_popup(&mut self) {
        if self.approval.overlay_visible {
            self.input_state.slash_popup.close();
            return;
        }
        let Some(query) = self.input_state.slash_query() else {
            self.input_state.slash_popup.close();
            return;
        };
        self.input_state
            .slash_popup
            .refresh(&query, &self.command_catalog);
    }

    pub fn enqueue_approval_request(
        &mut self,
        request_id: String,
        turn_id: u64,
        target: String,
        reason: String,
    ) {
        self.approval.add_request(ApprovalRequestUi {
            request_id,
            turn_id,
            target,
            reason,
        });
        self.input_state.input_enabled = false;
        self.input_state.slash_popup.close();
    }

    pub fn close_approval_overlay(&mut self, request_id: &str) {
        self.approval.resolve_request(request_id);
        if !self.approval.overlay_visible {
            self.input_state.input_enabled = true;
        }
    }

    pub fn submit_current_input(&mut self) -> Option<String> {
        self.input_state.consume_submitted()
    }

    pub fn handle_approval_selection(&mut self, selection: ApprovalSelection) {
        let Some(id) = self.approval.focused_request_id() else {
            return;
        };
        match selection {
            ApprovalSelection::Approve => {
                self.pending_decisions.push_back((id.clone(), true));
                self.close_approval_overlay(&id);
            }
            ApprovalSelection::Deny => {
                self.pending_decisions.push_back((id.clone(), false));
                self.close_approval_overlay(&id);
            }
            ApprovalSelection::Cancel => {
                self.approval.overlay_visible = false;
                self.approval.overlay = None;
                self.push_system_message("approval deferred".to_string());
            }
        }
    }

    pub fn take_pending_decision(&mut self) -> Option<(String, bool)> {
        self.pending_decisions.pop_front()
    }

    pub fn update_phase(&mut self, phase: TurnPhase) {
        self.turn.phase = phase;
    }
}
