use std::io::Write;

use crossterm::cursor::{MoveTo, RestorePosition, SavePosition};
use crossterm::style::{
    Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::{Clear, ClearType};
use crossterm::{execute, queue};
use ratatui::layout::Rect;
use ratatui::style::Style;
use unicode_width::UnicodeWidthStr;

use crate::history_cell::HistoryCell;

pub fn insert_history_cells<W: Write>(
    out: &mut W,
    area: Rect,
    cursor: (u16, u16),
    cells: &[HistoryCell],
) -> anyhow::Result<()> {
    if cells.is_empty() || area.width == 0 || area.height == 0 {
        return Ok(());
    }

    let top = area.top();
    let bottom = area.bottom().saturating_sub(1);
    if top >= bottom {
        return Ok(());
    }

    execute!(out, SavePosition)?;
    queue!(out, Print(format!("\x1b[{};{}r", top + 1, bottom + 1)))?;
    execute!(out, MoveTo(area.left(), bottom))?;

    for cell in cells {
        for line in &cell.lines {
            for visual_line in wrap_line(line, area.width) {
                queue!(out, Print("\r\n"), Clear(ClearType::CurrentLine))?;
                write_style(out, line.style)?;
                for span in &visual_line.spans {
                    write_style(out, span.style)?;
                    queue!(out, Print(span.content.as_ref()))?;
                    reset_style(out)?;
                }
                reset_style(out)?;
            }
        }
    }

    queue!(out, Print("\x1b[r"))?;
    execute!(out, RestorePosition)?;
    execute!(out, MoveTo(cursor.0, cursor.1))?;
    out.flush()?;
    Ok(())
}

fn wrap_line(line: &ratatui::text::Line<'static>, width: u16) -> Vec<ratatui::text::Line<'static>> {
    if width == 0 {
        return vec![line.clone()];
    }
    let mut result: Vec<ratatui::text::Line<'static>> = Vec::new();
    let mut current = Vec::new();
    let mut current_w = 0usize;
    let max_w = width as usize;

    for span in &line.spans {
        let mut remaining = span.content.to_string();
        while !remaining.is_empty() {
            let take = take_fit_prefix(&remaining, max_w.saturating_sub(current_w));
            if take.is_empty() {
                if !current.is_empty() {
                    result.push(ratatui::text::Line::from(current));
                    current = Vec::new();
                    current_w = 0;
                    continue;
                }
                let first_char = remaining.chars().next().unwrap_or_default().to_string();
                current.push(ratatui::text::Span::styled(first_char.clone(), span.style));
                remaining = remaining[first_char.len()..].to_string();
                result.push(ratatui::text::Line::from(current));
                current = Vec::new();
                current_w = 0;
                continue;
            }
            current_w += UnicodeWidthStr::width(take.as_str());
            current.push(ratatui::text::Span::styled(take.clone(), span.style));
            remaining = remaining[take.len()..].to_string();
            if current_w >= max_w {
                result.push(ratatui::text::Line::from(current));
                current = Vec::new();
                current_w = 0;
            }
        }
    }

    if !current.is_empty() {
        result.push(ratatui::text::Line::from(current));
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
        let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + w > remaining_width {
            break;
        }
        width += w;
        out.push(ch);
    }
    out
}

fn write_style<W: Write>(out: &mut W, style: Style) -> anyhow::Result<()> {
    if let Some(fg) = style.fg {
        queue!(out, SetForegroundColor(to_color(fg)))?;
    }
    if let Some(bg) = style.bg {
        queue!(out, SetBackgroundColor(to_color(bg)))?;
    }
    if style.add_modifier.contains(ratatui::style::Modifier::BOLD) {
        queue!(out, SetAttribute(Attribute::Bold))?;
    }
    if style.add_modifier.contains(ratatui::style::Modifier::DIM) {
        queue!(out, SetAttribute(Attribute::Dim))?;
    }
    Ok(())
}

fn reset_style<W: Write>(out: &mut W) -> anyhow::Result<()> {
    queue!(
        out,
        SetAttribute(Attribute::Reset),
        ResetColor,
        SetForegroundColor(Color::Reset),
        SetBackgroundColor(Color::Reset)
    )?;
    Ok(())
}

fn to_color(color: ratatui::style::Color) -> Color {
    match color {
        ratatui::style::Color::Black => Color::Black,
        ratatui::style::Color::Red => Color::DarkRed,
        ratatui::style::Color::Green => Color::DarkGreen,
        ratatui::style::Color::Yellow => Color::DarkYellow,
        ratatui::style::Color::Blue => Color::DarkBlue,
        ratatui::style::Color::Magenta => Color::DarkMagenta,
        ratatui::style::Color::Cyan => Color::DarkCyan,
        ratatui::style::Color::Gray => Color::Grey,
        ratatui::style::Color::DarkGray => Color::DarkGrey,
        ratatui::style::Color::LightRed => Color::Red,
        ratatui::style::Color::LightGreen => Color::Green,
        ratatui::style::Color::LightYellow => Color::Yellow,
        ratatui::style::Color::LightBlue => Color::Blue,
        ratatui::style::Color::LightMagenta => Color::Magenta,
        ratatui::style::Color::LightCyan => Color::Cyan,
        ratatui::style::Color::White => Color::White,
        ratatui::style::Color::Reset => Color::Reset,
        ratatui::style::Color::Rgb(r, g, b) => Color::Rgb { r, g, b },
        ratatui::style::Color::Indexed(idx) => Color::AnsiValue(idx),
    }
}
