use ratatui::style::{Color, Style};

pub fn title_style() -> Style {
    Style::default().fg(Color::Cyan)
}

pub fn role_style(role: &str) -> Style {
    match role {
        "user" => Style::default().fg(Color::LightBlue),
        "assistant" => Style::default().fg(Color::LightGreen),
        "system" => Style::default().fg(Color::DarkGray),
        _ => Style::default().fg(Color::White),
    }
}
