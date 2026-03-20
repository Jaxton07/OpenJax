use openjax_protocol::Event;
use tui_next::app::App;

#[test]
fn tool_started_renders_shell_command_target() {
    let mut app = App::default();
    app.apply_core_event(Event::ToolCallStarted {
        turn_id: 1,
        tool_call_id: "tc_1".to_string(),
        tool_name: "shell".to_string(),
        target: Some("ps aux | head -5".to_string()),
        display_name: None,
    });

    let cells = app.drain_history_cells();
    assert_eq!(cells.len(), 1);
    let text = cells[0]
        .lines
        .iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("Run shell (ps aux | head -5)"));
}
