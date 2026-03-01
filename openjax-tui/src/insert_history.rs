use crate::custom_terminal::TerminalState;
use crossterm::Command;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Attribute as CAttribute;
use crossterm::style::Color as CColor;
use crossterm::style::Colors;
use crossterm::style::Print;
use crossterm::style::SetAttribute;
use crossterm::style::SetColors;
use crossterm::terminal::{Clear, ClearType};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use std::fmt;
use std::io;
use std::io::Write;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub fn insert_history_lines<W: Write>(
    writer: &mut W,
    state: &mut TerminalState,
    lines: Vec<Line<'static>>,
) -> io::Result<()> {
    if lines.is_empty() {
        return Ok(());
    }

    let screen_height = state.last_known_screen_size.height;
    if screen_height == 0 {
        return Ok(());
    }

    let mut area = state.viewport_area;
    if area.width == 0 {
        return Ok(());
    }
    if area.height == 0 {
        area.height = 1;
    }

    let wrap_width = usize::from(area.width.max(1));
    let wrapped = wrap_lines(lines, wrap_width);
    let wrapped_lines = wrapped.len() as u16;

    let cursor_top = if area.bottom() < screen_height {
        let scroll_amount = wrapped_lines.min(screen_height - area.bottom());
        let top_1based = area.top() + 1;
        queue!(writer, SetScrollRegion(top_1based..screen_height))?;
        queue!(writer, MoveTo(0, area.top()))?;
        for _ in 0..scroll_amount {
            queue!(writer, Print("\x1bM"))?;
        }
        queue!(writer, ResetScrollRegion)?;
        area.y += scroll_amount;
        state.set_viewport_area(area);
        area.top().saturating_sub(1)
    } else {
        area.top().saturating_sub(1)
    };

    if area.top() <= 1 {
        state.restore_cursor(writer)?;
        return Ok(());
    }
    queue!(writer, SetScrollRegion(1..area.top()))?;
    queue!(writer, MoveTo(0, cursor_top))?;

    for line in wrapped {
        queue!(
            writer,
            Print("\r\n"),
            SetColors(Colors::new(
                line.style.fg.map(crossterm_color).unwrap_or(CColor::Reset),
                line.style.bg.map(crossterm_color).unwrap_or(CColor::Reset)
            )),
            Clear(ClearType::UntilNewLine)
        )?;
        let merged_spans: Vec<Span<'static>> = line
            .spans
            .iter()
            .map(|s| Span {
                style: s.style.patch(line.style),
                content: s.content.clone(),
            })
            .collect();
        write_spans(writer, merged_spans.iter())?;
    }

    queue!(writer, ResetScrollRegion)?;
    state.restore_cursor(writer)?;
    Ok(())
}

fn wrap_lines(lines: Vec<Line<'static>>, width: usize) -> Vec<Line<'static>> {
    let width = width.max(1);
    let mut out = Vec::new();
    for line in lines {
        if line.spans.is_empty() {
            out.push(Line::from("").style(line.style));
            continue;
        }

        let mut current_spans: Vec<Span<'static>> = Vec::new();
        let mut current_width = 0usize;

        for span in line.spans {
            let patched_style = span.style.patch(line.style);
            for grapheme in span.content.as_ref().graphemes(true) {
                let grapheme_width = UnicodeWidthStr::width(grapheme);
                if current_width > 0 && current_width + grapheme_width > width {
                    out.push(Line::from(std::mem::take(&mut current_spans)).style(line.style));
                    current_width = 0;
                }

                if grapheme_width > width && current_width == 0 {
                    out.push(Line::from(vec![Span::styled(
                        grapheme.to_string(),
                        patched_style,
                    )]));
                    continue;
                }

                if let Some(last) = current_spans.last_mut()
                    && last.style == patched_style
                {
                    last.content.to_mut().push_str(grapheme);
                } else {
                    current_spans.push(Span::styled(grapheme.to_string(), patched_style));
                }
                current_width += grapheme_width;
            }
        }

        if current_spans.is_empty() {
            out.push(Line::from("").style(line.style));
        } else {
            out.push(Line::from(current_spans).style(line.style));
        }
    }
    out
}

fn write_spans<'a>(
    writer: &mut impl Write,
    spans: impl IntoIterator<Item = &'a Span<'a>>,
) -> io::Result<()> {
    let mut prev_style = Style::default();
    for span in spans {
        let next_style = span.style;
        queue!(writer, SetColors(style_to_colors(next_style)))?;
        ModifierDiff {
            from: prev_style.add_modifier,
            to: next_style.add_modifier,
        }
        .queue(writer)?;
        queue!(writer, Print(span.content.as_ref()))?;
        prev_style = next_style;
    }
    queue!(writer, SetColors(Colors::new(CColor::Reset, CColor::Reset)))?;
    ModifierDiff {
        from: prev_style.add_modifier,
        to: Modifier::empty(),
    }
    .queue(writer)?;
    Ok(())
}

fn style_to_colors(style: Style) -> Colors {
    Colors::new(
        style.fg.map(crossterm_color).unwrap_or(CColor::Reset),
        style.bg.map(crossterm_color).unwrap_or(CColor::Reset),
    )
}

fn crossterm_color(color: Color) -> CColor {
    match color {
        Color::Reset => CColor::Reset,
        Color::Black => CColor::Black,
        Color::Red => CColor::DarkRed,
        Color::Green => CColor::DarkGreen,
        Color::Yellow => CColor::DarkYellow,
        Color::Blue => CColor::DarkBlue,
        Color::Magenta => CColor::DarkMagenta,
        Color::Cyan => CColor::DarkCyan,
        Color::Gray => CColor::Grey,
        Color::DarkGray => CColor::DarkGrey,
        Color::LightRed => CColor::Red,
        Color::LightGreen => CColor::Green,
        Color::LightYellow => CColor::Yellow,
        Color::LightBlue => CColor::Blue,
        Color::LightMagenta => CColor::Magenta,
        Color::LightCyan => CColor::Cyan,
        Color::White => CColor::White,
        Color::Indexed(v) => CColor::AnsiValue(v),
        Color::Rgb(r, g, b) => CColor::Rgb { r, g, b },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SetScrollRegion(std::ops::Range<u16>);

impl Command for SetScrollRegion {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x1b[{};{}r", self.0.start, self.0.end)
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(io::Error::other(
            "SetScrollRegion requires ANSI-capable terminal",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResetScrollRegion;

impl Command for ResetScrollRegion {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x1b[r")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(io::Error::other(
            "ResetScrollRegion requires ANSI-capable terminal",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

struct ModifierDiff {
    from: Modifier,
    to: Modifier,
}

impl ModifierDiff {
    fn queue(&self, writer: &mut impl Write) -> io::Result<()> {
        let removed = self.from - self.to;
        if removed.contains(Modifier::REVERSED) {
            queue!(writer, SetAttribute(CAttribute::NoReverse))?;
        }
        if removed.contains(Modifier::BOLD) {
            queue!(writer, SetAttribute(CAttribute::NormalIntensity))?;
            if self.to.contains(Modifier::DIM) {
                queue!(writer, SetAttribute(CAttribute::Dim))?;
            }
        }
        if removed.contains(Modifier::ITALIC) {
            queue!(writer, SetAttribute(CAttribute::NoItalic))?;
        }
        if removed.contains(Modifier::UNDERLINED) {
            queue!(writer, SetAttribute(CAttribute::NoUnderline))?;
        }
        if removed.contains(Modifier::DIM) {
            queue!(writer, SetAttribute(CAttribute::NormalIntensity))?;
        }
        if removed.contains(Modifier::CROSSED_OUT) {
            queue!(writer, SetAttribute(CAttribute::NotCrossedOut))?;
        }
        if removed.contains(Modifier::SLOW_BLINK) || removed.contains(Modifier::RAPID_BLINK) {
            queue!(writer, SetAttribute(CAttribute::NoBlink))?;
        }

        let added = self.to - self.from;
        if added.contains(Modifier::REVERSED) {
            queue!(writer, SetAttribute(CAttribute::Reverse))?;
        }
        if added.contains(Modifier::BOLD) {
            queue!(writer, SetAttribute(CAttribute::Bold))?;
        }
        if added.contains(Modifier::ITALIC) {
            queue!(writer, SetAttribute(CAttribute::Italic))?;
        }
        if added.contains(Modifier::UNDERLINED) {
            queue!(writer, SetAttribute(CAttribute::Underlined))?;
        }
        if added.contains(Modifier::DIM) {
            queue!(writer, SetAttribute(CAttribute::Dim))?;
        }
        if added.contains(Modifier::CROSSED_OUT) {
            queue!(writer, SetAttribute(CAttribute::CrossedOut))?;
        }
        if added.contains(Modifier::SLOW_BLINK) {
            queue!(writer, SetAttribute(CAttribute::SlowBlink))?;
        }
        if added.contains(Modifier::RAPID_BLINK) {
            queue!(writer, SetAttribute(CAttribute::RapidBlink))?;
        }
        Ok(())
    }
}
