use openjax_protocol::{Event, ShellExecutionMetadata};
use tui_next::app::App;

#[test]
fn degraded_mutating_shell_output_shows_high_risk_warning() {
    let mut app = App::default();
    app.apply_core_event(Event::ToolCallCompleted {
        turn_id: 1,
        tool_call_id: "tc_1".to_string(),
        tool_name: "shell".to_string(),
        display_name: None,
        ok: true,
        output: "command=git add -A && git commit -m \"x\"\nstdout:\nok\nstderr:\n".to_string(),
        shell_metadata: Some(ShellExecutionMetadata {
            result_class: "success".to_string(),
            backend: "none_escalated".to_string(),
            exit_code: 0,
            policy_decision: "AskApproval".to_string(),
            runtime_allowed: true,
            degrade_reason: Some("macos_seatbelt: denied".to_string()),
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
    assert!(text.contains("sandbox: none (degraded)"));
    assert!(text.contains("risk: mutating command ran unsandboxed"));
}
