use tui::app::App;

#[test]
fn desired_height_has_floor() {
    let app = App::default();
    assert!(app.desired_height(10) >= 6);
}
