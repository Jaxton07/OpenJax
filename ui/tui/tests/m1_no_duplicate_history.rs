use tui_next::app::App;

#[test]
fn submitted_message_is_not_duplicated_in_pending_queue() {
    let mut app = App::default();
    app.initialize_banner_once();
    let _banner = app.drain_history_cells();

    app.append_input("hello");
    let _ = app.submit_input();

    let cells = app.drain_history_cells();
    assert_eq!(cells.len(), 1);
}
