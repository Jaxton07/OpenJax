use openjax_protocol::Event;
use tui_next::app::App;
use tui_next::history_cell::CellRole;

#[test]
fn assistant_final_message_is_committed_to_history_cells() {
    let mut app = App::default();
    app.initialize_banner_once();
    let _ = app.drain_history_cells();

    app.apply_core_event(Event::TurnStarted { turn_id: 7 });
    app.apply_core_event(Event::AssistantDelta {
        turn_id: 7,
        content_delta: "第一行".to_string(),
    });

    // Delta only updates live area and should not commit into history.
    assert!(app.drain_history_cells().is_empty());

    app.apply_core_event(Event::AssistantMessage {
        turn_id: 7,
        content: "第一行\n第二行".to_string(),
    });

    let committed = app.drain_history_cells();
    assert_eq!(committed.len(), 1);
    assert_eq!(committed[0].role, CellRole::Assistant);
    assert!(committed[0].lines.len() >= 2);
    let first_line = committed[0].lines[0].to_string();
    assert!(first_line.contains("第一行"));
}
