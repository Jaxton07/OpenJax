use openjax_protocol::{Event, StreamSource};
use tui_next::app::App;
use tui_next::history_cell::CellRole;

#[test]
fn assistant_final_message_is_committed_to_history_cells() {
    let mut app = App::default();
    app.initialize_banner_once();
    let _ = app.drain_history_cells();

    app.apply_core_event(Event::TurnStarted { turn_id: 7 });
    assert!(app.drain_history_cells().is_empty());

    app.apply_core_event(Event::AssistantMessage {
        turn_id: 7,
        content: "第一行\n第二行".to_string(),
    });

    // Legacy assistant_message should seed stream text only; commit happens on turn completion.
    assert!(app.drain_history_cells().is_empty());

    app.apply_core_event(Event::TurnCompleted { turn_id: 7 });

    // Final commit should persist assistant content into history once.
    let committed = app.drain_history_cells();
    assert_eq!(committed.len(), 1);
    assert_eq!(committed[0].role, CellRole::Assistant);
    assert!(committed[0].lines.len() >= 2);
    let first_line = committed[0].lines[0].to_string();
    assert!(first_line.contains("第一行"));
}

#[test]
fn response_completed_remains_authoritative_when_legacy_assistant_message_arrives_later() {
    let mut app = App::default();
    app.initialize_banner_once();
    let _ = app.drain_history_cells();

    app.apply_core_event(Event::TurnStarted { turn_id: 9 });
    app.apply_core_event(Event::ResponseCompleted {
        turn_id: 9,
        content: "final-from-response-completed".to_string(),
        stream_source: StreamSource::Synthetic,
    });
    app.apply_core_event(Event::AssistantMessage {
        turn_id: 9,
        content: "legacy-late-message".to_string(),
    });
    app.apply_core_event(Event::TurnCompleted { turn_id: 9 });

    let committed = app.drain_history_cells();
    assert_eq!(committed.len(), 1);
    assert_eq!(committed[0].role, CellRole::Assistant);
    let line = committed[0].lines[0].to_string();
    assert!(line.contains("final-from-response-completed"));
}
