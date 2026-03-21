use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use openjax_protocol::Event as CoreEvent;
use tui_next::app::App;
use tui_next::history_cell::CellRole;
use tui_next::input::{InputAction, map_event};

#[test]
fn slash_palette_navigation_takes_priority_over_history() {
    let mut app = App::default();
    app.append_input("first message");
    let _ = app.submit_input();
    app.append_input("/");
    assert!(app.is_slash_palette_active());
    assert_eq!(app.state.slash_palette.selected_index, 0);
    assert!(app.state.slash_palette.matches.len() > 1);

    app.move_slash_selection(1);

    assert_eq!(app.state.input, "/");
    assert_eq!(app.state.slash_palette.selected_index, 1);
}

#[test]
fn approval_request_dismisses_slash_palette() {
    let mut app = App::default();
    app.append_input("/he");
    assert!(app.is_slash_palette_active());

    app.apply_core_event(CoreEvent::ApprovalRequested {
        turn_id: 1,
        request_id: "req-1".to_string(),
        target: "write file".to_string(),
        reason: "needs approval".to_string(),
        tool_name: Some("shell".to_string()),
        command_preview: Some("touch file".to_string()),
        risk_tags: vec!["write".to_string()],
        sandbox_backend: Some("linux_native".to_string()),
        degrade_reason: None,
    });

    assert!(!app.is_slash_palette_active());
}

#[test]
fn paste_like_append_refreshes_palette_once() {
    let mut app = App::default();
    app.append_input("/he");
    assert!(app.is_slash_palette_active());
    assert_eq!(app.state.slash_palette.matches[0].command_name, "help");
}

#[test]
fn tab_and_escape_map_to_slash_actions() {
    let tab = map_event(Event::Key(KeyEvent::new(
        KeyCode::Tab,
        KeyModifiers::empty(),
    )));
    let esc = map_event(Event::Key(KeyEvent::new(
        KeyCode::Esc,
        KeyModifiers::empty(),
    )));

    assert_eq!(tab, InputAction::AcceptSuggestion);
    assert_eq!(esc, InputAction::DismissOverlay);
}

#[test]
fn char_input_only_appends_without_modifiers_or_with_shift() {
    let plain = map_event(Event::Key(KeyEvent::new(
        KeyCode::Char('a'),
        KeyModifiers::empty(),
    )));
    let shifted = map_event(Event::Key(KeyEvent::new(
        KeyCode::Char('A'),
        KeyModifiers::SHIFT,
    )));
    let ctrl = map_event(Event::Key(KeyEvent::new(
        KeyCode::Char('a'),
        KeyModifiers::CONTROL,
    )));

    assert_eq!(plain, InputAction::Append("a".to_string()));
    assert_eq!(shifted, InputAction::Append("A".to_string()));
    assert_eq!(ctrl, InputAction::None);
}

#[test]
fn release_events_are_ignored_and_ctrl_c_still_quits() {
    let release = map_event(Event::Key(KeyEvent::new_with_kind(
        KeyCode::Char('a'),
        KeyModifiers::empty(),
        KeyEventKind::Release,
    )));
    let ctrl_c = map_event(Event::Key(KeyEvent::new(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL,
    )));

    assert_eq!(release, InputAction::None);
    assert_eq!(ctrl_c, InputAction::Quit);
}

#[test]
fn first_completion_does_not_execute_local_command() {
    let mut app = App::default();
    app.append_input("hello");
    let _ = app.submit_input();
    app.append_input("/cl");
    let history_count_before = app.state.history_cells.len();

    let result = app.complete_slash_selection();

    assert_eq!(result, tui_next::app::SlashAcceptResult::CompletedInput);
    assert_eq!(app.state.input, "/clear");
    assert_eq!(app.state.history_cells.len(), history_count_before);
}

#[test]
fn second_enter_executes_exact_local_command() {
    let mut app = App::default();
    app.append_input("hello");
    let _ = app.submit_input();
    app.append_input("/clear");

    let action = app.submit_input();

    assert!(action.is_none());
    assert_eq!(app.state.input, "");
    assert!(
        app.state
            .history_cells
            .iter()
            .any(|cell| matches!(cell.role, CellRole::Banner))
    );
}

#[test]
fn help_requires_second_enter_to_execute_builtin_action() {
    let mut app = App::default();
    app.append_input("/he");

    let result = app.complete_slash_selection();

    assert_eq!(result, tui_next::app::SlashAcceptResult::CompletedInput);
    assert_eq!(app.state.input, "/help");
    assert!(!app.is_slash_palette_active());

    let action = app.submit_input();
    assert!(action.is_none());
    assert_eq!(app.state.input, "");
}

#[test]
fn trailing_space_after_exact_command_allows_single_enter_submit() {
    let mut app = App::default();
    app.append_input("hello");
    let _ = app.submit_input();
    app.append_input("/clear ");

    assert!(!app.is_slash_palette_active());
    let action = app.submit_input();

    assert!(action.is_none());
    assert_eq!(app.state.input, "");
    assert!(
        app.state
            .history_cells
            .iter()
            .any(|cell| matches!(cell.role, CellRole::Banner))
    );
}
