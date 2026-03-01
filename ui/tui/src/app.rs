use openjax_protocol::Event;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::history_cell::{CellRole, HistoryCell};
use crate::state::{AppState, ApprovalSelection, LiveMessage, PendingApproval};

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

    pub fn desired_height(&self, width: u16) -> u16 {
        let footer_h = 1u16;
        let input_h = 2u16;
        let approval_h = if self.state.pending_approval.is_some() {
            self.approval_panel_height()
        } else {
            0
        };
        self.live_visual_height(width)
            .saturating_add(input_h)
            .saturating_add(approval_h)
            .saturating_add(footer_h)
            .max(8)
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

    pub fn apply_core_event(&mut self, event: Event) {
        match event {
            Event::TurnStarted { turn_id } => {
                self.state.active_turn_id = Some(turn_id);
                self.state.stream_turn_id = None;
                self.state.stream_text.clear();
                self.set_live_status("Thinking...");
            }
            Event::AssistantDelta {
                turn_id,
                content_delta,
            } => {
                if self.state.stream_turn_id != Some(turn_id) {
                    self.state.stream_turn_id = Some(turn_id);
                    self.state.stream_text.clear();
                }
                self.state.stream_text.push_str(&content_delta);
                self.state.live_messages = vec![LiveMessage {
                    role: "assistant",
                    content: self.state.stream_text.clone(),
                }];
            }
            Event::AssistantMessage { turn_id, content } => {
                self.state.stream_turn_id = Some(turn_id);
                self.state.stream_text = content.clone();
                let cell = self.assistant_cell(&content);
                self.queue_history_cell(cell);
                self.state.last_assistant_committed_turn = Some(turn_id);
                self.state.live_messages.clear();
            }
            Event::ToolCallStarted {
                tool_name, target, ..
            } => {
                let suffix = target.unwrap_or_default();
                let cell = self.tool_cell(if suffix.is_empty() {
                    format!("Run {}", tool_name)
                } else {
                    format!("Run {} ({})", tool_name, suffix)
                });
                self.queue_history_cell(cell);
            }
            Event::ToolCallCompleted {
                tool_name,
                ok,
                output,
                ..
            } => {
                let cell = self.tool_completed_cell(&tool_name, ok, &output);
                self.queue_history_cell(cell);
            }
            Event::ApprovalRequested {
                request_id,
                target,
                reason,
                ..
            } => {
                self.state.pending_approval = Some(PendingApproval {
                    request_id,
                    target,
                    reason,
                });
                self.state.approval_selection = ApprovalSelection::Approve;
                if let Some(pending) = &self.state.pending_approval {
                    self.state.live_messages = vec![LiveMessage {
                        role: "approval",
                        content: format!(
                            "{} - {} (input y/n + Enter)",
                            pending.target, pending.reason
                        ),
                    }];
                }
            }
            Event::ApprovalResolved {
                request_id,
                approved,
                ..
            } => {
                self.state.pending_approval = None;
                let cell = self.system_cell(format!(
                    "approval resolved {} ({})",
                    if approved { "approved" } else { "rejected" },
                    request_id
                ));
                self.queue_history_cell(cell);
            }
            Event::TurnCompleted { turn_id } => {
                self.state.active_turn_id = None;
                if self.state.stream_turn_id == Some(turn_id)
                    && !self.state.stream_text.is_empty()
                    && self.state.last_assistant_committed_turn != Some(turn_id)
                {
                    let content = self.state.stream_text.clone();
                    let cell = self.assistant_cell(&content);
                    self.queue_history_cell(cell);
                    self.state.last_assistant_committed_turn = Some(turn_id);
                }
                self.state.stream_text.clear();
                self.state.live_messages.clear();
            }
            Event::ShutdownComplete => {
                self.set_live_status("Shutdown complete");
            }
            Event::AgentSpawned { .. } | Event::AgentStatusChanged { .. } => {}
        }
    }

    pub fn render_live(&self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let paragraph = Paragraph::new(self.live_chat_lines()).wrap(Wrap { trim: false });
        let total_lines = self.live_visual_height(area.width) as usize;
        let scroll = total_lines.saturating_sub(area.height as usize);
        let paragraph = paragraph.scroll((scroll.min(u16::MAX as usize) as u16, 0));
        paragraph.render(area, buf);
    }

    pub fn live_chat_lines(&self) -> Vec<Line<'static>> {
        self.state
            .live_messages
            .iter()
            .flat_map(|message| {
                let mut local = Vec::new();
                for (idx, raw_line) in message.content.lines().enumerate() {
                    let prefix = if idx == 0 {
                        format!("{} ", message.role)
                    } else {
                        "  ".to_string()
                    };
                    local.push(Line::from(vec![
                        Span::styled(prefix, Style::default().fg(Color::DarkGray)),
                        Span::raw(raw_line.to_string()),
                    ]));
                }
                if local.is_empty() {
                    local.push(Line::from(Span::styled(
                        format!("{} ...", message.role),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
                local
            })
            .collect()
    }

    pub fn input_line(&self) -> Line<'static> {
        let mut spans = vec![Span::styled(
            "› ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )];
        spans.push(Span::raw(self.state.input.clone()));
        if self.state.input.is_empty() {
            spans.push(Span::styled("_", Style::default().fg(Color::DarkGray)));
        }
        Line::from(spans)
    }

    pub fn input_cursor_offset(&self, area_width: u16) -> u16 {
        let width = UnicodeWidthStr::width(self.state.input.as_str()) as u16;
        let raw = 2u16.saturating_add(width);
        raw.min(area_width.saturating_sub(1))
    }

    pub fn footer_text(&self) -> String {
        format!(
            "Enter submit | / commands | Ctrl-C quit || model={} | approval={} | sandbox={}",
            self.state.model_name.as_deref().unwrap_or("unknown"),
            self.state.approval_policy.as_deref().unwrap_or("unknown"),
            self.state.sandbox_mode.as_deref().unwrap_or("unknown"),
        )
    }

    pub fn move_approval_selection(&mut self, delta: i8) {
        if self.state.pending_approval.is_none() {
            return;
        }
        let current = match self.state.approval_selection {
            ApprovalSelection::Approve => 0i8,
            ApprovalSelection::Deny => 1i8,
            ApprovalSelection::Later => 2i8,
        };
        let next = (current + delta).rem_euclid(3);
        self.state.approval_selection = ApprovalSelection::from_index(next as usize);
    }

    pub fn approval_panel_lines(&self) -> Option<Vec<Line<'static>>> {
        let pending = self.state.pending_approval.as_ref()?;
        let mut lines = Vec::new();
        lines.push(Line::from(Span::styled(
            format!("是否允许执行操作: {}", pending.target),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            pending.reason.clone(),
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(self.approval_option_line(
            "Approve",
            self.state.approval_selection == ApprovalSelection::Approve,
        ));
        lines.push(self.approval_option_line(
            "Deny",
            self.state.approval_selection == ApprovalSelection::Deny,
        ));
        lines.push(self.approval_option_line(
            "Cancel or decide later",
            self.state.approval_selection == ApprovalSelection::Later,
        ));
        Some(lines)
    }

    pub fn approval_panel_height(&self) -> u16 {
        if self.state.pending_approval.is_some() {
            5
        } else {
            0
        }
    }

    pub fn set_live_status(&mut self, text: impl Into<String>) {
        self.state.live_messages = vec![LiveMessage {
            role: "status",
            content: text.into(),
        }];
    }

    fn live_visual_height(&self, width: u16) -> u16 {
        if self.state.live_messages.is_empty() {
            return 1;
        }
        let max_w = width.max(8) as usize;
        let mut total = 0usize;
        for line in self.live_chat_lines() {
            total += visual_line_count(&line, max_w);
        }
        total.max(1) as u16
    }

    fn queue_history_cell(&mut self, cell: HistoryCell) {
        self.state.history_cells.push(cell.clone());
        self.state.pending_history_cells.push(cell);
    }

    fn approval_option_line(&self, label: &str, selected: bool) -> Line<'static> {
        let marker = if selected { "› " } else { "  " };
        let style = if selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        Line::from(vec![
            Span::styled(marker, style),
            Span::styled(label.to_string(), style),
        ])
    }

    fn user_cell(&mut self, input: &str) -> HistoryCell {
        HistoryCell {
            id: self.alloc_id(),
            role: CellRole::User,
            committed: true,
            lines: vec![Line::from(vec![
                Span::styled(
                    "› ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(input.to_string()),
            ])],
        }
    }

    fn assistant_cell(&mut self, content: &str) -> HistoryCell {
        let mut lines = Vec::new();
        for (idx, line) in content.lines().enumerate() {
            if idx == 0 {
                lines.push(Line::from(vec![
                    Span::styled(
                        "● ",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(line.to_string()),
                ]));
            } else {
                lines.push(Line::from(format!("  {}", line)));
            }
        }
        if lines.is_empty() {
            lines.push(Line::from(Span::styled(
                "●",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )));
        }
        HistoryCell {
            id: self.alloc_id(),
            role: CellRole::Assistant,
            committed: true,
            lines,
        }
    }

    fn tool_cell(&mut self, content: String) -> HistoryCell {
        HistoryCell {
            id: self.alloc_id(),
            role: CellRole::Tool,
            committed: true,
            lines: vec![Line::from(vec![
                Span::styled(
                    "• ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(content),
            ])],
        }
    }

    fn tool_completed_cell(&mut self, tool_name: &str, ok: bool, output: &str) -> HistoryCell {
        let mut lines = Vec::new();
        let status = if ok {
            format!("{} completed", tool_name)
        } else {
            format!("{} failed", tool_name)
        };
        lines.push(Line::from(vec![
            Span::styled(
                "• ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(status),
        ]));

        for (idx, preview) in summarize_tool_output(output).into_iter().enumerate() {
            let prefix = if idx == 0 { "  └ " } else { "    " };
            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(Color::DarkGray)),
                Span::styled(preview, Style::default().fg(Color::Gray)),
            ]));
        }

        HistoryCell {
            id: self.alloc_id(),
            role: CellRole::Tool,
            committed: true,
            lines,
        }
    }

    fn system_cell(&mut self, content: String) -> HistoryCell {
        HistoryCell {
            id: self.alloc_id(),
            role: CellRole::System,
            committed: true,
            lines: vec![Line::from(vec![Span::styled(
                content,
                Style::default().fg(Color::DarkGray),
            )])],
        }
    }

    fn alloc_id(&mut self) -> u64 {
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

fn summarize_tool_output(output: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for raw_line in output.lines() {
        for segment in split_embedded_line_markers(raw_line) {
            let cleaned = strip_leading_line_marker(segment.trim()).trim();
            if cleaned.is_empty() {
                continue;
            }
            lines.push(truncate_chars(cleaned, 96));
        }
    }

    if lines.is_empty() {
        return vec!["(no output)".to_string()];
    }
    if lines.len() <= 4 {
        return lines;
    }

    vec![
        lines[0].clone(),
        format!("… +{} lines", lines.len().saturating_sub(2)),
        lines.last().cloned().unwrap_or_default(),
    ]
}

fn split_embedded_line_markers(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut starts = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'L' && (i == 0 || bytes[i - 1].is_ascii_whitespace()) {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 && j < bytes.len() && bytes[j] == b':' {
                starts.push(i);
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }

    if starts.len() <= 1 {
        return vec![text];
    }

    let mut out = Vec::new();
    for idx in 0..starts.len() {
        let start = starts[idx];
        let end = if idx + 1 < starts.len() {
            starts[idx + 1]
        } else {
            bytes.len()
        };
        out.push(text[start..end].trim());
    }
    out
}

fn strip_leading_line_marker(text: &str) -> &str {
    let bytes = text.as_bytes();
    if bytes.first() != Some(&b'L') {
        return text;
    }
    let mut idx = 1usize;
    while idx < bytes.len() && bytes[idx].is_ascii_digit() {
        idx += 1;
    }
    if idx == 1 || idx >= bytes.len() || bytes[idx] != b':' {
        return text;
    }
    idx += 1;
    while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
        idx += 1;
    }
    &text[idx..]
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out = String::new();
    for ch in text.chars().take(max_chars) {
        out.push(ch);
    }
    out.push_str("...");
    out
}

fn visual_line_count(line: &Line<'static>, max_w: usize) -> usize {
    if max_w == 0 {
        return 1;
    }
    let mut lines = 1usize;
    let mut current = 0usize;

    for span in &line.spans {
        for ch in span.content.chars() {
            let ch_w = UnicodeWidthChar::width(ch).unwrap_or(0).max(1);
            if current + ch_w > max_w {
                lines += 1;
                current = 0;
            }
            current += ch_w;
        }
    }

    lines
}
