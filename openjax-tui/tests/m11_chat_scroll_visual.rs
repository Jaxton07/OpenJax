use openjax_tui::app::App;
use openjax_tui::app_event::AppEvent;
use openjax_tui::state::RenderKind;

#[test]
fn follow_output_scroll_uses_wrapped_visual_line_count() {
    let mut app = App::default();
    app.state.push_assistant_message(
        "这是一个用于测试自动换行的很长中文段落这是一个用于测试自动换行的很长中文段落这是一个用于测试自动换行的很长中文段落 TAIL".to_string(),
        RenderKind::Plain,
    );

    let visual_bottom = app.chat_scroll_for_viewport(18, 4);
    let logical_bottom = openjax_tui::chatwidget::ChatWidget::render_lines(&app.state)
        .len()
        .saturating_sub(4);
    assert!(visual_bottom > logical_bottom);
}

#[test]
fn page_up_disables_follow_until_end() {
    let mut app = App::default();
    app.handle_event(AppEvent::ScrollPageUp);
    assert!(!app.state.transcript.follow_output);

    app.handle_event(AppEvent::ScrollBottom);
    assert!(app.state.transcript.follow_output);
}
