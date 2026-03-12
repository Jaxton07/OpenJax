use openjax_protocol::Event;
use tui_next::app::App;

#[test]
fn tool_completed_renders_partial_status() {
    let mut app = App::default();
    app.apply_core_event(Event::ToolCallCompleted {
        turn_id: 1,
        tool_call_id: "tc_1".to_string(),
        tool_name: "shell".to_string(),
        ok: true,
        output: "result_class=partial_success\ncommand=yes | head -n 1\nexit_code=141\nbackend=macos_seatbelt".to_string(),
    });

    let cells = app.drain_history_cells();
    assert_eq!(cells.len(), 1);
    let text = cells[0]
        .lines
        .iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("shell partial"));
    assert!(text.contains("sandbox: sandbox-exec (macos_seatbelt)"));
}
