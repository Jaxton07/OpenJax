use crossterm::event::KeyCode;
use openjax_protocol::Event;

#[derive(Debug, Clone)]
pub enum AppEvent {
    TuiKey(KeyCode),
    InputChar(char),
    InputPaste(String),
    Backspace,
    MoveCursorLeft,
    MoveCursorRight,
    HistoryPrev,
    HistoryNext,
    ScrollPageUp,
    ScrollPageDown,
    ScrollTop,
    ScrollBottom,
    SubmitInput,
    ToggleHelp,
    Escape,
    Tick,
    Redraw,
    CoreEvent(Event),
    Quit,
}
