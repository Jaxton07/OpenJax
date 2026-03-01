use crossterm::cursor::{Hide, Show};
use crossterm::event::{
    DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyModifiers, poll, read,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::Backend;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Position, Rect};
use ratatui::text::Line;
use std::io::Stdout;
use std::io::{self, Write};

use crate::app_event::AppEvent;
use crate::custom_terminal::TerminalState;
use crate::insert_history;

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
        AltScreenMode::Auto => false,
    }
}

pub fn enter_terminal_mode(mode: AltScreenMode) -> anyhow::Result<bool> {
    let alt_enabled = should_enable_alt_screen(mode);
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    if alt_enabled {
        execute!(stdout, EnterAlternateScreen, Hide, EnableBracketedPaste)?;
    } else {
        execute!(stdout, Hide, EnableBracketedPaste)?;
    }
    stdout.flush()?;
    Ok(alt_enabled)
}

pub fn restore_terminal_mode(alt_enabled: bool) -> anyhow::Result<()> {
    let mut stdout = io::stdout();
    execute!(stdout, Show, DisableBracketedPaste)?;
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

pub struct Tui {
    pub terminal: Terminal<CrosstermBackend<Stdout>>,
    pub terminal_state: TerminalState,
    pending_history_lines: Vec<Line<'static>>,
}

impl Tui {
    pub fn new(mut terminal: Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<Self> {
        let size = terminal.size()?;
        let cursor_pos = terminal
            .backend_mut()
            .get_cursor_position()
            .unwrap_or(Position { x: 0, y: 0 });
        let terminal_state = TerminalState::new(size, cursor_pos);
        Ok(Self {
            terminal,
            terminal_state,
            pending_history_lines: Vec::new(),
        })
    }

    pub fn insert_history_lines(&mut self, lines: Vec<Line<'static>>) {
        self.pending_history_lines.extend(lines);
    }

    pub fn clear_pending_history_lines(&mut self) {
        self.pending_history_lines.clear();
    }

    pub fn draw(
        &mut self,
        _height: u16,
        draw_fn: impl FnOnce(&mut ratatui::Frame<'_>, Rect),
    ) -> anyhow::Result<()> {
        let size = self.terminal.size()?;
        self.terminal_state.update_from_backend_size(size);
        self.terminal_state
            .update_cursor_from_backend(self.terminal.backend_mut());

        if !self.pending_history_lines.is_empty() {
            let lines = std::mem::take(&mut self.pending_history_lines);
            let backend = self.terminal.backend_mut();
            insert_history::insert_history_lines(backend, &mut self.terminal_state, lines)?;
            std::io::Write::flush(backend)?;
            self.terminal_state
                .update_cursor_from_backend(self.terminal.backend_mut());
        }

        let completed = self.terminal.draw(|frame| {
            let area = frame.area();
            draw_fn(frame, area);
        })?;
        self.terminal_state.set_viewport_area(completed.area);
        self.terminal_state
            .update_cursor_from_backend(self.terminal.backend_mut());
        Ok(())
    }
}

pub fn map_crossterm_event(event: Event) -> Option<AppEvent> {
    match event {
        Event::Paste(text) => Some(AppEvent::InputPaste(text)),
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
