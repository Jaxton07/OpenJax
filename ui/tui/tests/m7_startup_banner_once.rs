use tui::app::App;

#[test]
fn banner_is_queued_only_once() {
    let mut app = App::default();
    app.initialize_banner_once();
    app.initialize_banner_once();
    let cells = app.drain_history_cells();
    assert_eq!(cells.len(), 1);
}
