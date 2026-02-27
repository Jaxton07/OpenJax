use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

pub trait Renderable {
    fn render(&self, area: Rect, buf: &mut Buffer);
    fn desired_height(&self, _width: u16) -> u16 {
        1
    }
    fn cursor_pos(&self, _area: Rect) -> Option<(u16, u16)> {
        None
    }
}
