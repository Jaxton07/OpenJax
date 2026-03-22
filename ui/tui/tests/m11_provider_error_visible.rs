use openjax_protocol::Event;
use tui_next::app::App;

#[test]
fn provider_response_error_stays_visible_after_turn_complete() {
    let mut app = App::default();
    app.apply_core_event(Event::TurnStarted { turn_id: 11 });
    app.apply_core_event(Event::ResponseError {
        turn_id: 11,
        code: "model_request_failed".to_string(),
        message: "provider request failed: upstream provider returned 404".to_string(),
        retryable: false,
    });
    app.apply_core_event(Event::TurnCompleted { turn_id: 11 });

    let cells = app.drain_history_cells();
    assert!(
        cells.iter().any(|cell| {
            cell.lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .any(|span| span.content.contains("provider request failed"))
        }),
        "provider request failure should remain visible in committed history"
    );
}
