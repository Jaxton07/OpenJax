use openjax_protocol::Event;

#[derive(Debug, Clone)]
pub enum AppEvent {
    InputChar(char),
    Backspace,
    SubmitInput,
    CoreEvent(Event),
    Quit,
}
