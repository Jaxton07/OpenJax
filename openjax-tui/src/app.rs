use crate::app_event::AppEvent;
use crate::state::AppState;
use crate::ui::{chat_view, composer, logo, status_bar};
use ratatui::Frame;
use ratatui::layout::Alignment;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

#[derive(Debug, Default)]
pub struct App {
    pub state: AppState,
    should_quit: bool,
}

impl App {
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::InputChar(ch) => self.state.insert_input_char(ch),
            AppEvent::Backspace => self.state.backspace_input(),
            AppEvent::MoveCursorLeft => self.state.move_cursor_left(),
            AppEvent::MoveCursorRight => self.state.move_cursor_right(),
            AppEvent::HistoryPrev => self.state.recall_prev_history(),
            AppEvent::HistoryNext => self.state.recall_next_history(),
            AppEvent::ScrollPageUp => {
                self.state.chat_scroll = self.state.chat_scroll.saturating_sub(10);
                self.state.follow_output = false;
            }
            AppEvent::ScrollPageDown => {
                self.state.chat_scroll = self.state.chat_scroll.saturating_add(10);
                self.state.follow_output = false;
            }
            AppEvent::ScrollTop => {
                self.state.chat_scroll = 0;
                self.state.follow_output = false;
            }
            AppEvent::ScrollBottom => {
                self.state.chat_scroll = usize::MAX;
                self.state.follow_output = true;
            }
            AppEvent::SubmitInput => {
                if let Some(input) = self.state.consume_submitted_input() {
                    self.state.push_user_message(input);
                }
            }
            AppEvent::ToggleHelp => self.state.show_help = !self.state.show_help,
            AppEvent::CoreEvent(event) => self.state.map_core_event(&event),
            AppEvent::Quit => self.should_quit = true,
        }
    }

    pub fn render(&self, frame: &mut Frame<'_>) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(frame.area());

        let logo = Paragraph::new(logo::render_lines()).alignment(Alignment::Center);
        frame.render_widget(logo, chunks[0]);

        let chat_lines = chat_view::render_lines(&self.state);
        let chat_inner_height = chunks[1].height as usize;
        let max_scroll = chat_lines.len().saturating_sub(chat_inner_height);
        let scroll = if self.state.follow_output {
            max_scroll
        } else {
            self.state.chat_scroll.min(max_scroll)
        };
        let chat = Paragraph::new(chat_lines)
            .wrap(Wrap { trim: false })
            .scroll((scroll as u16, 0));
        frame.render_widget(chat, chunks[1]);

        let composer = Paragraph::new(vec![composer::render_line(&self.state)]);
        frame.render_widget(composer, chunks[2]);
        if chunks[2].width > 0 && chunks[2].height > 0 {
            let cursor_x = chunks[2].x + composer::cursor_offset(&self.state, chunks[2].width);
            let cursor_x = cursor_x.min(chunks[2].x + chunks[2].width.saturating_sub(1));
            frame.set_cursor_position((cursor_x, chunks[2].y));
        }

        let status = Paragraph::new(vec![status_bar::render_line(&self.state)]);
        frame.render_widget(status, chunks[3]);

        if let Some(overlay) = &self.state.approval_overlay {
            let popup = centered_rect(70, 25, frame.area());
            frame.render_widget(Clear, popup);
            frame.render_widget(
                Paragraph::new(overlay.prompt.clone())
                    .block(Block::default().title("Approval").borders(Borders::ALL))
                    .wrap(Wrap { trim: false }),
                popup,
            );
        }

        if self.state.show_help {
            let popup = centered_rect(70, 35, frame.area());
            frame.render_widget(Clear, popup);
            frame.render_widget(
                Paragraph::new(
                    "Shortcuts:\n\
Enter: submit input\n\
Backspace: delete char\n\
Left / Right: move cursor\n\
Up / Down: input history\n\
PageUp / PageDown: scroll chat\n\
Home / End: chat top/bottom\n\
?: toggle this help\n\
Ctrl-C: quit",
                )
                .block(Block::default().title("Help").borders(Borders::ALL))
                .wrap(Wrap { trim: false }),
                popup,
            );
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
