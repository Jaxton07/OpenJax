use std::io::Stdout;

use anyhow::Context;
use crossterm::cursor::position;
use ratatui::backend::Backend;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use ratatui::{Frame, Terminal, TerminalOptions, Viewport};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub struct CustomTerminal {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    pub viewport_area: Rect,
    pub last_known_screen_size: (u16, u16),
    pub last_known_cursor_pos: (u16, u16),
}

impl CustomTerminal {
    pub fn new(stdout: Stdout, viewport_height: u16) -> anyhow::Result<Self> {
        let mut terminal = Terminal::with_options(
            CrosstermBackend::new(stdout),
            TerminalOptions {
                viewport: Viewport::Inline(viewport_height.max(8)),
            },
        )?;
        terminal.hide_cursor()?;
        let area = terminal.get_frame().area();
        let cursor = position().unwrap_or((0, 0));
        Ok(Self {
            terminal,
            viewport_area: area,
            last_known_screen_size: (area.width, area.height),
            last_known_cursor_pos: cursor,
        })
    }

    pub fn set_viewport_area(&mut self, area: Rect) {
        self.viewport_area = area;
        self.last_known_screen_size = (area.width, area.height);
    }

    pub fn draw<F>(&mut self, mut draw_fn: F) -> anyhow::Result<()>
    where
        F: FnMut(&mut Frame<'_>, Rect),
    {
        let area = self.viewport_area;
        self.terminal
            .draw(|frame| draw_fn(frame, area))
            .context("custom terminal draw failed")?;
        self.update_cursor_from_backend();
        Ok(())
    }

    pub fn flush(&mut self) -> anyhow::Result<()> {
        self.terminal
            .backend_mut()
            .flush()
            .context("terminal flush failed")?;
        self.update_cursor_from_backend();
        Ok(())
    }

    pub fn backend_mut(&mut self) -> &mut CrosstermBackend<Stdout> {
        self.terminal.backend_mut()
    }

    pub fn insert_before_lines(
        &mut self,
        lines: &[Line<'static>],
        width: u16,
    ) -> anyhow::Result<()> {
        if width == 0 || lines.is_empty() {
            return Ok(());
        }

        let wrapped = wrap_lines(lines, width as usize);
        for line in wrapped {
            self.terminal
                .insert_before(1, |buf| {
                    let paragraph = Paragraph::new(line.clone());
                    paragraph.render(buf.area, buf);
                })
                .context("insert_before failed")?;
        }
        let area = self.terminal.get_frame().area();
        self.viewport_area = area;
        self.last_known_screen_size = (area.width, area.height);
        self.update_cursor_from_backend();
        Ok(())
    }

    pub fn update_cursor_from_backend(&mut self) {
        if let Ok(cursor) = position() {
            self.last_known_cursor_pos = cursor;
        }
    }

    pub fn area(&self) -> Rect {
        self.viewport_area
    }
}

fn wrap_lines(lines: &[Line<'static>], width: usize) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    for line in lines {
        out.extend(wrap_line(line, width));
    }
    out
}

fn wrap_line(line: &Line<'static>, width: usize) -> Vec<Line<'static>> {
    if width == 0 {
        return vec![line.clone()];
    }
    let mut result: Vec<Line<'static>> = Vec::new();
    let mut current: Vec<Span<'static>> = Vec::new();
    let mut current_w = 0usize;

    for span in &line.spans {
        let mut remaining = span.content.to_string();
        while !remaining.is_empty() {
            let take = take_fit_prefix(&remaining, width.saturating_sub(current_w));
            if take.is_empty() {
                if !current.is_empty() {
                    result.push(Line::from(std::mem::take(&mut current)));
                    current_w = 0;
                    continue;
                }
                let ch = remaining.chars().next().unwrap_or_default().to_string();
                current.push(Span::styled(ch.clone(), span.style));
                remaining = remaining[ch.len()..].to_string();
                result.push(Line::from(std::mem::take(&mut current)));
                current_w = 0;
                continue;
            }
            current_w += UnicodeWidthStr::width(take.as_str());
            current.push(Span::styled(take.clone(), span.style));
            remaining = remaining[take.len()..].to_string();
            if current_w >= width {
                result.push(Line::from(std::mem::take(&mut current)));
                current_w = 0;
            }
        }
    }
    if !current.is_empty() {
        result.push(Line::from(current));
    }
    if result.is_empty() {
        result.push(line.clone());
    }
    result
}

fn take_fit_prefix(text: &str, remaining_width: usize) -> String {
    if remaining_width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut width = 0usize;
    for ch in text.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0).max(1);
        if width + w > remaining_width {
            break;
        }
        width += w;
        out.push(ch);
    }
    out
}
