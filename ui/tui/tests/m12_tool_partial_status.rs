use openjax_protocol::{Event, ShellExecutionMetadata};
use tui_next::app::App;

#[test]
fn tool_completed_renders_partial_status() {
    let mut app = App::default();
    app.apply_core_event(Event::ToolCallCompleted {
        turn_id: 1,
        tool_call_id: "tc_1".to_string(),
        tool_name: "shell".to_string(),
        display_name: Some("Run Shell".to_string()),
        ok: true,
        output: "stdout:\nok\nstderr:\n".to_string(),
        shell_metadata: Some(ShellExecutionMetadata {
            result_class: "partial_success".to_string(),
            backend: "macos_seatbelt".to_string(),
            exit_code: 141,
            policy_decision: "allow".to_string(),
            runtime_allowed: true,
            degrade_reason: None,
            runtime_deny_reason: None,
        }),
    });

    let cells = app.drain_history_cells();
    assert_eq!(cells.len(), 1);
    let text = cells[0]
        .lines
        .iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("Run Shell partial"));
    assert!(text.contains("sandbox: sandbox-exec (macos_seatbelt)"));
}
