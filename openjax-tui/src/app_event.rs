use openjax_protocol::Event;

#[derive(Debug, Clone)]
pub enum AppEvent {
    InputChar(char),
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
    CoreEvent(Event),
    Quit,
}
