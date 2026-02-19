use openjax_protocol::Event;

#[derive(Debug, Clone)]
pub enum AppEvent {
    InputChar(char),
    Backspace,
    MoveCursorLeft,
    MoveCursorRight,
    HistoryPrev,
    HistoryNext,
    SubmitInput,
    ToggleHelp,
    CoreEvent(Event),
    Quit,
}
