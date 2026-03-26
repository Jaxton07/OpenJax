use tui_next::app::{App, FooterMode};
use tui_next::state::{PendingApproval, PolicyPickerState};

fn make_pending_approval() -> PendingApproval {
    PendingApproval {
        request_id: "r1".to_string(),
        target: "some tool".to_string(),
        reason: "needs approval".to_string(),
        tool_name: None,
        command_preview: None,
        risk_tags: vec![],
        sandbox_backend: None,
        degrade_reason: None,
    }
}

#[test]
fn open_policy_picker_with_no_pending_approval() {
    let mut app = App::default();
    app.state.policy_default = Some("ask".to_string());
    app.open_policy_picker();
    let picker = app
        .state
        .policy_picker
        .as_ref()
        .expect("picker should open");
    assert_eq!(picker.selected_index, 1, "ask maps to index 1");
}

#[test]
fn open_policy_picker_blocked_by_pending_approval() {
    let mut app = App::default();
    app.state.pending_approval = Some(make_pending_approval());
    app.open_policy_picker();
    assert!(
        app.state.policy_picker.is_none(),
        "picker must not open during approval"
    );
}

#[test]
fn move_policy_selection_wraps() {
    let mut app = App::default();
    app.state.policy_picker = Some(PolicyPickerState { selected_index: 0 });
    app.move_policy_selection(-1);
    assert_eq!(
        app.state.policy_picker.as_ref().unwrap().selected_index,
        2,
        "0 - 1 wraps to 2"
    );
    app.move_policy_selection(1);
    assert_eq!(
        app.state.policy_picker.as_ref().unwrap().selected_index,
        0,
        "2 + 1 wraps to 0"
    );
}

#[test]
fn apply_policy_pick_updates_policy_default_and_clears_picker() {
    let mut app = App::default();
    app.state.policy_picker = Some(PolicyPickerState { selected_index: 0 });
    app.apply_policy_pick("allow");
    assert_eq!(app.state.policy_default, Some("allow".to_string()));
    assert!(app.state.policy_picker.is_none());
}

#[test]
fn dismiss_policy_picker_clears_picker_without_changing_policy() {
    let mut app = App::default();
    app.state.policy_default = Some("ask".to_string());
    app.state.policy_picker = Some(PolicyPickerState { selected_index: 2 });
    app.dismiss_policy_picker();
    assert!(app.state.policy_picker.is_none());
    assert_eq!(app.state.policy_default, Some("ask".to_string()));
}

#[test]
fn footer_line_contains_correct_policy_label() {
    let mut app = App::default();
    for (input, expected) in [
        ("allow", "allow"),
        ("ask", "ask"),
        ("deny", "deny"),
        ("unknown", "ask"), // fallback
    ] {
        app.state.policy_default = Some(input.to_string());
        let line = app.footer_line();
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            text.contains(expected),
            "footer for '{input}' should contain '{expected}', got: {text}"
        );
    }
}

#[test]
fn policy_picker_lines_highlights_correct_index() {
    let mut app = App::default();
    app.state.policy_picker = Some(PolicyPickerState { selected_index: 2 }); // deny
    let lines = app
        .policy_picker_lines()
        .expect("picker lines should exist");
    // 2 header lines + 3 options = 5 lines; option at index 2 is lines[4]
    let strict_line_text: String = lines[4].spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        strict_line_text.contains("deny"),
        "last option line should contain 'deny', got: {strict_line_text}"
    );
    // leading marker should be '› ' for selected
    assert!(
        lines[4].spans[0].content.contains('›'),
        "selected option should have '›' marker"
    );
}

#[test]
fn footer_mode_is_policy_picker_active_when_picker_open() {
    let mut app = App::default();
    app.state.policy_picker = Some(PolicyPickerState { selected_index: 1 });
    let layout = app.bottom_layout(80);
    assert_eq!(layout.footer_mode, FooterMode::PolicyPickerActive);
}
