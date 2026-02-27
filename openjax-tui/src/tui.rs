use crossterm::Command;
use crossterm::cursor::{Hide, MoveTo, RestorePosition, SavePosition, Show};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, poll, read};
use crossterm::execute;
use crossterm::queue;
use crossterm::style::Attribute as CAttribute;
use crossterm::style::Color as CColor;
use crossterm::style::Colors;
use crossterm::style::Print;
use crossterm::style::SetAttribute;
use crossterm::style::SetColors;
use crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::{Frame, Terminal};
use std::io::Stdout;
use std::io::{self, Write};
use std::ops::Range;

use crate::app_event::AppEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AltScreenMode {
    Auto,
    Always,
    Never,
}

impl AltScreenMode {
    pub fn from_env() -> Self {
        match std::env::var("OPENJAX_TUI_ALT_SCREEN")
            .unwrap_or_else(|_| "auto".to_string())
            .to_ascii_lowercase()
            .as_str()
        {
            "always" => Self::Always,
            "never" => Self::Never,
            _ => Self::Auto,
        }
    }
}

pub fn should_enable_alt_screen(mode: AltScreenMode) -> bool {
    match mode {
        AltScreenMode::Always => true,
        AltScreenMode::Never => false,
        AltScreenMode::Auto => std::env::var("ZELLIJ").is_err(),
    }
}

pub fn enter_terminal_mode(mode: AltScreenMode) -> anyhow::Result<bool> {
    let alt_enabled = should_enable_alt_screen(mode);
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    if alt_enabled {
        execute!(stdout, EnterAlternateScreen, Hide)?;
    } else {
        execute!(stdout, Hide)?;
    }
    stdout.flush()?;
    Ok(alt_enabled)
}

pub fn restore_terminal_mode(alt_enabled: bool) -> anyhow::Result<()> {
    let mut stdout = io::stdout();
    execute!(stdout, Show)?;
    if alt_enabled {
        execute!(stdout, LeaveAlternateScreen)?;
    }
    disable_raw_mode()?;
    stdout.flush()?;
    Ok(())
}

pub fn restore_plan(raw_enabled: bool, alt_enabled: bool) -> Vec<&'static str> {
    let mut ops = Vec::new();
    ops.push("show_cursor");
    if alt_enabled {
        ops.push("leave_alt_screen");
    }
    if raw_enabled {
        ops.push("disable_raw_mode");
    }
    ops
}

pub fn map_crossterm_event(event: Event) -> Option<AppEvent> {
    match event {
        Event::Key(KeyEvent {
            code: KeyCode::Char('\u{3}'),
            ..
        }) => Some(AppEvent::Quit),
        Event::Key(KeyEvent {
            code: KeyCode::Char('C'),
            modifiers,
            ..
        }) if modifiers.contains(KeyModifiers::CONTROL) => Some(AppEvent::Quit),
        Event::Key(KeyEvent {
            code: KeyCode::Char('c'),
            modifiers,
            ..
        }) if modifiers.contains(KeyModifiers::CONTROL) => Some(AppEvent::Quit),
        Event::Key(KeyEvent {
            code: KeyCode::Esc, ..
        }) => Some(AppEvent::Escape),
        Event::Key(KeyEvent {
            code: KeyCode::Char('?'),
            ..
        }) => Some(AppEvent::ToggleHelp),
        Event::Key(KeyEvent {
            code: KeyCode::Enter,
            ..
        }) => Some(AppEvent::SubmitInput),
        Event::Key(KeyEvent {
            code: KeyCode::Backspace,
            ..
        }) => Some(AppEvent::Backspace),
        Event::Key(KeyEvent {
            code: KeyCode::Left,
            ..
        }) => Some(AppEvent::MoveCursorLeft),
        Event::Key(KeyEvent {
            code: KeyCode::Right,
            ..
        }) => Some(AppEvent::MoveCursorRight),
        Event::Key(KeyEvent {
            code: KeyCode::Up, ..
        }) => Some(AppEvent::HistoryPrev),
        Event::Key(KeyEvent {
            code: KeyCode::Down,
            ..
        }) => Some(AppEvent::HistoryNext),
        Event::Key(KeyEvent {
            code: KeyCode::PageUp,
            ..
        }) => Some(AppEvent::ScrollPageUp),
        Event::Key(KeyEvent {
            code: KeyCode::PageDown,
            ..
        }) => Some(AppEvent::ScrollPageDown),
        Event::Key(KeyEvent {
            code: KeyCode::Home,
            ..
        }) => Some(AppEvent::ScrollTop),
        Event::Key(KeyEvent {
            code: KeyCode::End, ..
        }) => Some(AppEvent::ScrollBottom),
        Event::Key(KeyEvent {
            code: KeyCode::Char(ch),
            ..
        }) => Some(AppEvent::InputChar(ch)),
        _ => None,
    }
}

pub fn next_app_event() -> anyhow::Result<Option<AppEvent>> {
    if !poll(std::time::Duration::from_millis(50))? {
        return Ok(None);
    }
    let event = read()?;
    Ok(map_crossterm_event(event))
}

pub fn draw_with_height(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    height: u16,
    draw_fn: impl FnOnce(&mut Frame<'_>, Rect),
) -> anyhow::Result<()> {
    terminal.draw(|frame| {
        let area = frame.area();
        let view_h = height.clamp(1, area.height.max(1));
        let view = Rect::new(
            area.x,
            area.y + area.height.saturating_sub(view_h),
            area.width,
            view_h,
        );
        draw_fn(frame, view);
    })?;
    Ok(())
}

pub fn insert_history_lines(
    lines: &[Line<'static>],
    viewport_top: u16,
    screen_height: u16,
) -> anyhow::Result<()> {
    if lines.is_empty() || screen_height == 0 {
        return Ok(());
    }

    let mut stdout = io::stdout();
    if viewport_top == 0 {
        // Degenerate viewport: fall back to simple append instead of dropping history.
        for line in lines {
            queue!(stdout, Print("\r\n"))?;
            write_spans(&mut stdout, line.spans.iter())?;
        }
        stdout.flush()?;
        return Ok(());
    }
    queue!(stdout, SavePosition)?;
    queue!(
        stdout,
        SetScrollRegion(1..viewport_top),
        MoveTo(0, viewport_top.saturating_sub(1))
    )?;
    for line in lines {
        queue!(
            stdout,
            Print("\r\n"),
            SetColors(Colors::new(CColor::Reset, CColor::Reset)),
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
        write_spans(&mut stdout, merged_spans.iter())?;
    }
    queue!(stdout, ResetScrollRegion, RestorePosition)?;
    stdout.flush()?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetScrollRegion(pub Range<u16>);

impl Command for SetScrollRegion {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        write!(f, "\x1b[{};{}r", self.0.start, self.0.end)
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        Err(std::io::Error::other(
            "SetScrollRegion requires ANSI-capable terminal",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResetScrollRegion;

impl Command for ResetScrollRegion {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        write!(f, "\x1b[r")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        Err(std::io::Error::other(
            "ResetScrollRegion requires ANSI-capable terminal",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
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
