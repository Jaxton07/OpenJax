use openjax_protocol::Event;
use tui_next::app::App;

#[test]
fn shell_approval_panel_uses_compact_english_copy_with_countdown() {
    let mut app = App::default();
    app.apply_core_event(Event::ApprovalRequested {
        turn_id: 1,
        request_id: "req-1".to_string(),
        target: "ps aux --sort=-%cpu | head -10".to_string(),
        reason: "sandbox backend unavailable; fallback requires explicit approval".to_string(),
        tool_name: Some("shell".to_string()),
        command_preview: Some("ps aux --sort=-%cpu | head -10".to_string()),
        risk_tags: vec!["sandbox_degrade".to_string()],
        sandbox_backend: Some("macos_seatbelt".to_string()),
        degrade_reason: Some("runtime denied".to_string()),
        policy_version: None,
        matched_rule_id: None,
        approval_kind: None,
    });

    let lines = app.approval_panel_lines().expect("panel should exist");
    let text = lines
        .iter()
        .map(|l| l.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("Approval Required ("));
    assert!(text.contains("Command: ps aux --sort=-%cpu | head -10"));
    assert!(text.contains("Reason: Sandbox denied execution; fallback needs approval"));
    assert!(text.contains("Approve and run without sandbox"));
    assert!(text.contains("Deny this request"));
    assert!(text.contains("Decide later"));
}
