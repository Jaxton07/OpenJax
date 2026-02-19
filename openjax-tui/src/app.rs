use crate::app_event::AppEvent;
use crate::state::AppState;
use crate::ui::{chat_view, composer, status_bar};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::widgets::{Block, Borders, Paragraph};

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
        let chat = Paragraph::new(chat_lines)
            .block(Block::default().title("OpenJax TUI").borders(Borders::ALL));
        frame.render_widget(chat, chunks[0]);

        let composer = Paragraph::new(vec![composer::render_line(&self.state)])
            .block(Block::default().title("Input").borders(Borders::ALL));
        frame.render_widget(composer, chunks[1]);

        let status = Paragraph::new(vec![status_bar::render_line()]);
        frame.render_widget(status, chunks[2]);
    }
}
