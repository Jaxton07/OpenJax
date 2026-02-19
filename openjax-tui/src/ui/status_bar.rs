use ratatui::text::Line;

pub fn render_line(show_help: bool) -> Line<'static> {
    if show_help {
        Line::from("Enter submit | Backspace delete | ? hide help | q quit")
    } else {
        Line::from("Enter submit | ? help | q quit")
    }
}
