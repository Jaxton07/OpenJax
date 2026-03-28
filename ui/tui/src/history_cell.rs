use ratatui::style::Style;
use ratatui::text::{Line, Span};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CellRole {
    Banner,
    User,
    Assistant,
    Tool,
    System,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryCell {
    pub id: u64,
    pub role: CellRole,
    pub committed: bool,
    pub lines: Vec<Line<'static>>,
}

impl HistoryCell {
    pub fn from_plain(id: u64, role: CellRole, text: impl Into<String>) -> Self {
        let style = match role {
            CellRole::Banner => Style::default(),
            CellRole::User => Style::default(),
            CellRole::Assistant => Style::default(),
            CellRole::Tool => Style::default(),
            CellRole::System => Style::default(),
        };
        let lines = text
            .into()
            .lines()
            .map(|line| Line::from(Span::styled(line.to_string(), style)))
            .collect();
        Self {
            id,
            role,
            committed: true,
            lines,
        }
    }
}
