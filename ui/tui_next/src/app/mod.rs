use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::history_cell::{CellRole, HistoryCell};
use crate::state::{AppState, ApprovalSelection, LiveMessage};

mod cells;
mod layout_metrics;
mod reducer;
mod render_model;
mod tool_output;

#[derive(Debug, Default)]
pub struct App {
    pub state: AppState,
}

impl App {
    pub fn initialize_banner_once(&mut self) {
        if self.state.banner_printed {
            return;
        }
        self.state.banner_printed = true;
        let banner_id = self.alloc_id();
        self.queue_history_cell(HistoryCell {
            id: banner_id,
            role: CellRole::Banner,
            committed: true,
            lines: vec![
                Line::from(Span::styled(
                    "OPENJAX",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    "Personal Assistant",
                    Style::default()
                        .fg(Color::Gray)
                        .add_modifier(Modifier::BOLD | Modifier::DIM),
                )),
            ],
        });
    }

    pub fn set_runtime_info(
        &mut self,
        model_name: String,
        approval_policy: String,
        sandbox_mode: String,
    ) {
        self.state.model_name = Some(model_name);
        self.state.approval_policy = Some(approval_policy);
        self.state.sandbox_mode = Some(sandbox_mode);
    }

    pub fn drain_history_cells(&mut self) -> Vec<HistoryCell> {
        std::mem::take(&mut self.state.pending_history_cells)
    }

    pub fn submit_input(&mut self) -> Option<SubmitAction> {
        let input = self.state.input.trim().to_string();

        if let Some(pending) = self.state.pending_approval.clone() {
            let lower = input.to_ascii_lowercase();
            let selected = if input.is_empty() {
                Some(self.state.approval_selection)
            } else if matches!(lower.as_str(), "y" | "yes") {
                Some(ApprovalSelection::Approve)
            } else if matches!(lower.as_str(), "n" | "no") {
                Some(ApprovalSelection::Deny)
            } else if matches!(lower.as_str(), "l" | "later" | "cancel") {
                Some(ApprovalSelection::Later)
            } else {
                None
            };

            self.state.input.clear();
            match selected {
                Some(ApprovalSelection::Approve) | Some(ApprovalSelection::Deny) => {
                    let approved = matches!(selected, Some(ApprovalSelection::Approve));
                    self.state.pending_approval = None;
                    self.state.approval_selection = ApprovalSelection::Approve;
                    let cell = self.system_cell(format!(
                        "approval {} ({})",
                        if approved { "approved" } else { "rejected" },
                        pending.request_id
                    ));
                    self.queue_history_cell(cell);
                    return Some(SubmitAction::ApprovalDecision {
                        request_id: pending.request_id,
                        approved,
                    });
                }
                Some(ApprovalSelection::Later) => {
                    self.set_live_status("Approval pending: choose Approve or Deny when ready");
                    return None;
                }
                None => {
                    self.set_live_status("Invalid approval input. Use y/n/l or arrow keys + Enter");
                    return None;
                }
            }
        }

        if input.is_empty() {
            return None;
        }

        let user_cell = self.user_cell(&input);
        self.queue_history_cell(user_cell);
        self.state.input.clear();
        self.set_live_status("Thinking...");
        Some(SubmitAction::UserTurn { input })
    }

    pub fn append_input(&mut self, text: &str) {
        self.state.input.push_str(text);
    }

    pub fn backspace(&mut self) {
        self.state.input.pop();
    }

    pub fn clear(&mut self) {
        self.state.history_cells.clear();
        self.state.pending_history_cells.clear();
        self.state.live_messages.clear();
        self.state.input.clear();
        self.state.pending_approval = None;
        self.state.approval_selection = ApprovalSelection::Approve;
        self.state.active_turn_id = None;
        self.state.stream_turn_id = None;
        self.state.stream_text.clear();
        self.state.last_assistant_committed_turn = None;
        self.state.banner_printed = false;
        self.initialize_banner_once();
    }

    pub fn set_live_status(&mut self, text: impl Into<String>) {
        self.state.live_messages = vec![LiveMessage {
            role: "status",
            content: text.into(),
        }];
    }

    pub(crate) fn queue_history_cell(&mut self, cell: HistoryCell) {
        self.state.history_cells.push(cell.clone());
        self.state.pending_history_cells.push(cell);
    }

    pub(crate) fn alloc_id(&mut self) -> u64 {
        let id = self.state.next_cell_id;
        self.state.next_cell_id = self.state.next_cell_id.saturating_add(1);
        id
    }
}

#[derive(Debug)]
pub enum SubmitAction {
    UserTurn { input: String },
    ApprovalDecision { request_id: String, approved: bool },
}
