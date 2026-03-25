use openjax_protocol::Event as CoreEvent;
use tui_next::app::App;

fn footer_hint(app: &App) -> String {
    // Extract just the hint text (first span) from footer_line()
    app.footer_line()
        .spans
        .first()
        .map(|s| s.content.to_string())
        .unwrap_or_default()
}

#[test]
fn slash_toggle_keeps_desired_height_and_stable_footer() {
    let mut app = App::default();
    let base = app.desired_height(80);
    assert_eq!(
        footer_hint(&app),
        "Enter submit | / commands | Esc clear | Ctrl-C quit"
    );

    app.append_input("/");
    assert_eq!(app.desired_height(80), base);
    assert_eq!(footer_hint(&app), "Tab/Enter complete | Esc dismiss");

    app.backspace();
    assert_eq!(app.desired_height(80), base);
    assert_eq!(
        footer_hint(&app),
        "Enter submit | / commands | Esc clear | Ctrl-C quit"
    );
}

#[test]
fn approval_panel_keeps_desired_height_and_uses_short_footer() {
    let mut app = App::default();
    let base = app.desired_height(80);

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
        policy_version: None,
        matched_rule_id: None,
        approval_kind: None,
    });

    assert_eq!(app.desired_height(80), base);
    assert_eq!(footer_hint(&app), "↑↓ select | Enter confirm | Esc later");
}
