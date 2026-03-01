use tui_next::app::App;

#[test]
fn submit_commits_user_and_assistant_once() {
    let mut app = App::default();
    app.append_input("abc");
    let _ = app.submit_input();
    let first = app.drain_history_cells();
    assert_eq!(first.len(), 1);

    let second = app.drain_history_cells();
    assert!(second.is_empty());
}
