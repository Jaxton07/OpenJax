use openjax_protocol::Event;
use tui_next::app::App;

#[test]
fn degraded_mutating_shell_output_shows_high_risk_warning() {
    let mut app = App::default();
    app.apply_core_event(Event::ToolCallCompleted {
        turn_id: 1,
        tool_call_id: "tc_1".to_string(),
        tool_name: "shell".to_string(),
        ok: true,
        output: "result_class=success\ncommand=git add -A && git commit -m \"x\"\nexit_code=0\nbackend=none_escalated\ndegrade_reason=macos_seatbelt: denied\npolicy_decision=AskApproval\nruntime_allowed=true\nruntime_deny_reason=none\nstdout:\nok\nstderr:\n".to_string(),
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
