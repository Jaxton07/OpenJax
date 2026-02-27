use crossterm::cursor::{Hide, Show};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, poll, read};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::{Frame, Terminal};
use std::io::Stdout;
use std::io::{self, Write};

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
