use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::app_event::AppEvent;
use crate::bottom_pane::approval_overlay;
use crate::bottom_pane::chat_composer;
use crate::bottom_pane::footer;
use crate::chatwidget::{ChatWidget, visual_line_count};
use crate::state::{AppState, ApprovalSelection, apply_core_event};
use crate::ui::composer;

#[derive(Debug)]
pub struct App {
    pub state: AppState,
    should_quit: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: AppState::with_defaults(),
            should_quit: false,
        }
    }
}

impl App {
    const FOOTER_HEIGHT: u16 = 1;
    const COMPOSER_HEIGHT: u16 = 3;
    const COMPOSER_OVERLAY_HEIGHT: u16 = 8;
    const LIVE_TAIL_MESSAGES: usize = 2;

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn take_pending_approval_decision(&mut self) -> Option<(String, bool)> {
        self.state.take_pending_decision()
    }

    pub fn desired_height(&self, width: u16) -> u16 {
        let composer_height = self.composer_height();
        let chat_height = visual_line_count(&self.live_chat_lines(), width) as u16;
        chat_height
            .max(1)
            .saturating_add(composer_height)
            .saturating_add(Self::FOOTER_HEIGHT)
    }

    pub fn chat_scroll_for_viewport(&self, chat_width: u16, chat_height: u16) -> usize {
        let chat_lines = self.live_chat_lines();
        let chat_visual_lines = visual_line_count(&chat_lines, chat_width);
        let max_scroll = chat_visual_lines.saturating_sub(chat_height as usize);
        if self.state.transcript.follow_output {
            max_scroll
        } else {
            self.state.transcript.chat_scroll.min(max_scroll)
        }
    }

    pub fn live_chat_lines(&self) -> Vec<Line<'static>> {
        ChatWidget::render_live_lines(&self.state)
    }

    pub fn collect_new_history_lines_for_inline(&mut self) -> Vec<Line<'static>> {
        let total = self.state.transcript.messages.len();
        let emitted = self.state.history_emission.emitted_message_count.min(total);
        self.state.history_emission.emitted_message_count = emitted;

        let mut stable_end = emitted;
        let mut blocked_stream_turn = None;
        for message in &self.state.transcript.messages[emitted..] {
            let is_unstable_assistant = message.role == "assistant"
                && message.render_kind == crate::state::RenderKind::Plain
                && self.state.turn.active_turn_id.is_some();
            if is_unstable_assistant {
                blocked_stream_turn = self.state.turn.active_turn_id;
                break;
            }
            stable_end += 1;
        }
        self.state.history_emission.emitted_stream_turn_id = blocked_stream_turn;

        let emit_end = stable_end.saturating_sub(Self::LIVE_TAIL_MESSAGES);
        if emit_end <= emitted {
            return Vec::new();
        }

        let lines =
            ChatWidget::render_message_lines(&self.state.transcript.messages[emitted..emit_end]);
        self.state.history_emission.emitted_message_count = emit_end;
        if emit_end > 0 {
            self.state.history_emission.has_emitted_any = true;
        }
        lines
    }

    pub fn handle_event(&mut self, event: AppEvent) {
        if self.state.approval.overlay_visible {
            let is_core = matches!(&event, AppEvent::CoreEvent(_));
            match event {
                AppEvent::HistoryPrev => self.state.approval.move_selection(-1),
                AppEvent::HistoryNext => self.state.approval.move_selection(1),
                AppEvent::SubmitInput => {
                    if let Some(sel) = approval_overlay::confirm_selection(&self.state) {
                        self.state.handle_approval_selection(sel);
                    }
                }
                AppEvent::Escape => self
                    .state
                    .handle_approval_selection(ApprovalSelection::Cancel),
                AppEvent::InputChar('y') => self
                    .state
                    .handle_approval_selection(ApprovalSelection::Approve),
                AppEvent::InputChar('n') => self
                    .state
                    .handle_approval_selection(ApprovalSelection::Deny),
                AppEvent::CoreEvent(ref core) => apply_core_event(&mut self.state, core),
                AppEvent::Quit => self.should_quit = true,
                _ => {}
            }
            if !is_core {
                return;
            }
        }

        chat_composer::handle_input_event(&mut self.state, &event);
        match event {
            AppEvent::InputChar(_) | AppEvent::InputPaste(_) | AppEvent::Backspace => {}
            AppEvent::MoveCursorLeft | AppEvent::MoveCursorRight => {}
            AppEvent::HistoryPrev => {
                if self.state.input_state.slash_popup.open {
                    self.state.input_state.slash_popup.move_selection(-1);
                } else {
                    self.state.input_state.recall_prev();
                }
            }
            AppEvent::HistoryNext => {
                if self.state.input_state.slash_popup.open {
                    self.state.input_state.slash_popup.move_selection(1);
                } else {
                    self.state.input_state.recall_next();
                }
            }
            AppEvent::ScrollPageUp => {
                self.state.transcript.chat_scroll =
                    self.state.transcript.chat_scroll.saturating_sub(10);
                self.state.transcript.follow_output = false;
            }
            AppEvent::ScrollPageDown => {
                self.state.transcript.chat_scroll =
                    self.state.transcript.chat_scroll.saturating_add(10);
                self.state.transcript.follow_output = false;
            }
            AppEvent::ScrollTop => {
                self.state.transcript.chat_scroll = 0;
                self.state.transcript.follow_output = false;
            }
            AppEvent::ScrollBottom => {
                self.state.transcript.chat_scroll = usize::MAX;
                self.state.transcript.follow_output = true;
            }
            AppEvent::SubmitInput => self.submit_input_or_command(),
            AppEvent::Escape => self.state.input_state.slash_popup.close(),
            AppEvent::ToggleHelp => self.state.show_help = !self.state.show_help,
            AppEvent::CoreEvent(core) => apply_core_event(&mut self.state, &core),
            AppEvent::Quit => self.should_quit = true,
            AppEvent::Tick | AppEvent::Redraw | AppEvent::TuiKey(_) => {}
        }
    }

    pub fn render(&self, frame: &mut Frame<'_>) {
        self.render_in_area(frame, frame.area());
    }

    pub fn render_in_area(&self, frame: &mut Frame<'_>, area: Rect) {
        let composer_height = self.composer_height();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(composer_height),
                Constraint::Length(Self::FOOTER_HEIGHT),
            ])
            .split(area);

        let chat_lines = self.live_chat_lines();
        let scroll = self.chat_scroll_for_viewport(chunks[0].width, chunks[0].height);
        let scroll_y = scroll.min(u16::MAX as usize) as u16;
        let chat = Paragraph::new(chat_lines)
            .wrap(Wrap { trim: false })
            .scroll((scroll_y, 0));
        frame.render_widget(chat, chunks[0]);

        let composer_line = composer::render_line(&self.state);
        let composer_widget =
            Paragraph::new(composer_line).block(Block::default().borders(Borders::TOP));
        frame.render_widget(composer_widget, chunks[1]);
        let cursor_x = chunks[1]
            .x
            .saturating_add(composer::cursor_offset(&self.state, chunks[1].width))
            .min(chunks[1].x + chunks[1].width.saturating_sub(1));
        frame.set_cursor_position((cursor_x, chunks[1].y + 1));

        if self.state.input_state.slash_popup.open {
            let lines = self
                .state
                .input_state
                .slash_popup
                .filtered
                .iter()
                .enumerate()
                .map(|(idx, item)| {
                    let prefix = if idx == self.state.input_state.slash_popup.selected {
                        "› "
                    } else {
                        "  "
                    };
                    format!("{prefix}/{:<12} {}", item.name, item.description).into()
                })
                .collect::<Vec<ratatui::text::Line<'static>>>();

            let popup_h = (lines.len().max(1) as u16 + 2).min(chunks[1].height.saturating_sub(2));
            let rect = ratatui::layout::Rect {
                x: chunks[1].x,
                y: chunks[1].y + 2,
                width: chunks[1].width,
                height: popup_h,
            };
            frame.render_widget(Clear, rect);
            frame.render_widget(
                Paragraph::new(lines)
                    .style(Style::default().fg(Color::White))
                    .block(Block::default().title("Commands").borders(Borders::ALL)),
                rect,
            );
        }

        if self.state.approval.overlay_visible {
            let lines = approval_overlay::render_lines(&self.state);
            let popup_h = (lines.len().max(1) as u16 + 2).min(chunks[1].height.saturating_sub(2));
            let rect = ratatui::layout::Rect {
                x: chunks[1].x,
                y: chunks[1].y + 2,
                width: chunks[1].width,
                height: popup_h,
            };
            frame.render_widget(Clear, rect);
            frame.render_widget(
                Paragraph::new(lines)
                    .block(Block::default().title("Approval").borders(Borders::ALL))
                    .wrap(Wrap { trim: false }),
                rect,
            );
        }

        frame.render_widget(Paragraph::new(footer::render_line(&self.state)), chunks[2]);

        if self.state.show_help {
            let popup = centered_rect(70, 35, area);
            frame.render_widget(Clear, popup);
            frame.render_widget(
                Paragraph::new(
                    "Shortcuts:\n\
Enter: submit input\n\
Backspace: delete char\n\
Left / Right: move cursor\n\
Up / Down: history or popup\n\
PageUp / PageDown: scroll chat\n\
Home / End: chat top/bottom\n\
Esc: close popup\n\
Ctrl-C: quit",
                )
                .block(Block::default().title("Help").borders(Borders::ALL))
                .wrap(Wrap { trim: false }),
                popup,
            );
        }
    }

    fn composer_height(&self) -> u16 {
        if self.state.approval.overlay_visible || self.state.input_state.slash_popup.open {
            Self::COMPOSER_OVERLAY_HEIGHT
        } else {
            Self::COMPOSER_HEIGHT
        }
    }

    fn submit_input_or_command(&mut self) {
        if self.state.input_state.slash_popup.open {
            if let Some(cmd) = self.state.input_state.slash_popup.selected_command() {
                if cmd.enabled {
                    self.execute_slash(cmd.name);
                } else {
                    self.state
                        .push_system_message(format!("command /{} is not enabled yet", cmd.name));
                }
            } else {
                self.state
                    .push_system_message("no slash command match".to_string());
            }
            self.state.input_state.buffer.clear();
            self.state.input_state.cursor = 0;
            self.state.input_state.slash_popup.close();
            return;
        }

        if let Some(input) = self.state.submit_current_input() {
            self.state.push_user_message(input);
        }
    }

    fn execute_slash(&mut self, command_name: &str) {
        match command_name {
            "help" => self.state.show_help = true,
            "clear" => self.state.clear_messages(),
            "exit" => self.should_quit = true,
            "pending" => {
                let count = self.state.approval.pending_count();
                self.state
                    .push_system_message(format!("pending approvals: {count}"));
            }
            "approve" => self
                .state
                .handle_approval_selection(ApprovalSelection::Approve),
            "deny" => self
                .state
                .handle_approval_selection(ApprovalSelection::Deny),
            _ => self
                .state
                .push_system_message(format!("unknown command: /{command_name}")),
        }
    }
}

fn centered_rect(
    percent_x: u16,
    percent_y: u16,
    r: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
