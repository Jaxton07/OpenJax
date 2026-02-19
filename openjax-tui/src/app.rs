use crate::app_event::AppEvent;
use crate::render::theme;
use crate::state::AppState;
use crate::ui::{chat_view, composer, status_bar};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

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
            AppEvent::InputChar(ch) => self.state.input.push(ch),
            AppEvent::Backspace => {
                self.state.input.pop();
            }
            AppEvent::SubmitInput => {
                let input = self.state.input.trim().to_string();
                if !input.is_empty() {
                    self.state.push_user_message(input);
                    self.state.input.clear();
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
                Constraint::Min(1),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(frame.area());

        let chat_lines = chat_view::render_lines(&self.state);
        let chat = Paragraph::new(chat_lines).block(
            Block::default()
                .title(Span::styled("OpenJax TUI", theme::title_style()))
                .borders(Borders::ALL),
        );
        frame.render_widget(chat, chunks[0]);

        let composer = Paragraph::new(vec![composer::render_line(&self.state)])
            .block(Block::default().title("Input").borders(Borders::ALL));
        frame.render_widget(composer, chunks[1]);

        let status = Paragraph::new(vec![status_bar::render_line(self.state.show_help)]);
        frame.render_widget(status, chunks[2]);

        if let Some(overlay) = &self.state.approval_overlay {
            let popup = centered_rect(70, 25, frame.area());
            frame.render_widget(Clear, popup);
            frame.render_widget(
                Paragraph::new(overlay.prompt.clone())
                    .block(Block::default().title("Approval").borders(Borders::ALL)),
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
?: toggle this help\n\
q / Esc / Ctrl-C: quit",
                )
                .block(Block::default().title("Help").borders(Borders::ALL)),
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
