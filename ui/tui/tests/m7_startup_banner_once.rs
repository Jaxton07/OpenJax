use tui_next::app::App;
use tui_next::history_cell::CellRole;

#[test]
fn banner_is_queued_only_once() {
    let mut app = App::default();
    app.initialize_banner_once();
    app.initialize_banner_once();
    let cells = app.drain_history_cells();
    assert_eq!(cells.len(), 1);
}

#[test]
fn banner_contains_runtime_info_box_lines() {
    let mut app = App::default();
    app.set_runtime_info(
        "openai".to_string(),
        "on-request".to_string(),
        "workspace-write".to_string(),
        std::path::Path::new("/tmp/openjax"),
    );
    app.initialize_banner_once();

    let cells = app.drain_history_cells();
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].role, CellRole::Banner);
    let text = cells[0]
        .lines
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("OpenJax TUI"));
    assert!(text.contains("model:     openai"));
    assert!(text.contains("directory: /tmp/openjax"));
    assert!(text.contains("╭"));
    assert!(text.contains("╯"));
}
