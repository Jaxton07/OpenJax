use openjax_protocol::Event;
use tui_next::app::{App, SubmitAction};

#[test]
fn approval_history_is_written_only_on_resolved_event() {
    let mut app = App::default();
    app.apply_core_event(Event::ApprovalRequested {
        turn_id: 1,
        request_id: "req-123".to_string(),
        target: "git commit -m \"x\"".to_string(),
        reason: "approval required".to_string(),
        tool_name: Some("shell".to_string()),
        command_preview: Some("git commit -m \"x\"".to_string()),
        risk_tags: vec!["write".to_string()],
        sandbox_backend: Some("macos_seatbelt".to_string()),
        degrade_reason: None,
        policy_version: None,
        matched_rule_id: None,
    });

    app.state.input = "y".to_string();
    let action = app.submit_input();
    assert!(matches!(
        action,
        Some(SubmitAction::ApprovalDecision {
            request_id,
            approved: true
        }) if request_id == "req-123"
    ));
    assert!(
        app.drain_history_cells().is_empty(),
        "local approval input should not create duplicate history rows"
    );

    app.apply_core_event(Event::ApprovalResolved {
        turn_id: 1,
        request_id: "req-123".to_string(),
        approved: true,
    });
    let cells = app.drain_history_cells();
    assert_eq!(cells.len(), 1);
    let text = cells[0]
        .lines
        .iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("approval resolved approved (req-123)"));
}

#[test]
fn duplicate_approval_requested_with_same_id_is_deduped() {
    let mut app = App::default();
    app.apply_core_event(Event::ApprovalRequested {
        turn_id: 2,
        request_id: "req-dup".to_string(),
        target: "git add -A".to_string(),
        reason: "sandbox backend unavailable".to_string(),
        tool_name: None,
        command_preview: None,
        risk_tags: vec![],
        sandbox_backend: None,
        degrade_reason: None,
        policy_version: None,
        matched_rule_id: None,
    });
    let first_live = app.state.live_messages.clone();

    app.apply_core_event(Event::ApprovalRequested {
        turn_id: 2,
        request_id: "req-dup".to_string(),
        target: "git add -A".to_string(),
        reason: "sandbox backend unavailable; fallback requires explicit approval".to_string(),
        tool_name: Some("shell".to_string()),
        command_preview: Some("git add -A".to_string()),
        risk_tags: vec!["fs_write".to_string()],
        sandbox_backend: Some("macos_seatbelt".to_string()),
        degrade_reason: Some("permission denied".to_string()),
        policy_version: None,
        matched_rule_id: None,
    });

    let pending = app
        .state
        .pending_approval
        .as_ref()
        .expect("approval pending");
    assert_eq!(pending.request_id, "req-dup");
    assert_eq!(pending.tool_name.as_deref(), Some("shell"));
    assert_eq!(pending.command_preview.as_deref(), Some("git add -A"));
    assert_eq!(pending.sandbox_backend.as_deref(), Some("macos_seatbelt"));
    assert_eq!(app.state.live_messages, first_live);
    assert_eq!(app.drain_history_cells().len(), 0);
}

#[test]
fn approval_live_message_reason_is_single_line_summary() {
    let mut app = App::default();
    let long_reason = "sandbox backend unavailable; fallback requires explicit approval\n\n\
git: warning: confstr() failed with code 5\n\
git: error: could not open /dev/null";
    app.apply_core_event(Event::ApprovalRequested {
        turn_id: 3,
        request_id: "req-reason".to_string(),
        target: "git add -A".to_string(),
        reason: long_reason.to_string(),
        tool_name: Some("shell".to_string()),
        command_preview: Some("git add -A".to_string()),
        risk_tags: vec![],
        sandbox_backend: Some("macos_seatbelt".to_string()),
        degrade_reason: None,
        policy_version: None,
        matched_rule_id: None,
    });

    let message = app
        .state
        .live_messages
        .first()
        .map(|m| m.content.clone())
        .unwrap_or_default();
    assert!(!message.contains('\n'));
    assert!(message.contains("pending (req-reason)"));
    assert!(!message.contains("cmd="));
}
