use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};

pub fn render_lines() -> Vec<Line<'static>> {
    let colors = [
        Color::LightCyan,
        Color::LightBlue,
        Color::Blue,
        Color::Cyan,
        Color::LightGreen,
        Color::Yellow,
        Color::LightYellow,
    ];
    let word = "OPENJAX";
    let spans = word
        .chars()
        .enumerate()
        .map(|(idx, ch)| Span::styled(ch.to_string(), Style::default().fg(colors[idx]).bold()))
        .collect::<Vec<_>>();

    vec![
        Line::from(spans),
        Line::from(Span::styled(
            "Personal Assistant",
            Style::default().fg(Color::DarkGray),
        )),
    ]
}
