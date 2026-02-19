use ratatui::text::Line;

pub fn render_line() -> Line<'static> {
    Line::from("Enter: submit | q: quit")
}
