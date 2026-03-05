use tui_next::app::App;

#[test]
fn submit_input_sets_running_status_bar() {
    let mut app = App::default();
    app.append_input("hello");
    let _ = app.submit_input();

    let status = app.state.status_bar.as_ref().expect("status should exist");
    assert_eq!(status.label, "Working");
}
