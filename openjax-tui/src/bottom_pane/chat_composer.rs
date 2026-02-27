use crossterm::event::KeyCode;

use crate::app_event::AppEvent;
use crate::state::AppState;

pub fn handle_input_event(state: &mut AppState, event: &AppEvent) {
    match event {
        AppEvent::InputChar(ch) if state.input_state.input_enabled => {
            state.input_state.insert_char(*ch);
            state.sync_slash_popup();
        }
        AppEvent::Backspace if state.input_state.input_enabled => {
            state.input_state.backspace();
            state.sync_slash_popup();
        }
        AppEvent::MoveCursorLeft if state.input_state.input_enabled => {
            state.input_state.move_left();
        }
        AppEvent::MoveCursorRight if state.input_state.input_enabled => {
            state.input_state.move_right();
        }
        AppEvent::TuiKey(KeyCode::Esc) => {
            state.input_state.slash_popup.close();
        }
        _ => {}
    }
}
