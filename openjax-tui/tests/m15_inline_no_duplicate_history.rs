use openjax_tui::app::App;
use openjax_tui::state::RenderKind;

#[test]
fn inline_history_emits_once_and_live_area_excludes_emitted_messages() {
    let mut app = App::default();
    app.state.push_user_message("第一轮用户".to_string());
    app.state
        .push_assistant_message("第一轮助手".to_string(), RenderKind::Markdown);
    app.state.push_user_message("第二轮用户".to_string());
    app.state
        .push_assistant_message("第二轮助手".to_string(), RenderKind::Markdown);

    let first = app.collect_new_history_lines_for_inline();
    assert!(!first.is_empty());
    assert!(
        first
            .iter()
            .any(|line| line.to_string().contains("第一轮用户"))
    );
    assert!(
        first
            .iter()
            .any(|line| line.to_string().contains("第一轮助手"))
    );

    let second = app.collect_new_history_lines_for_inline();
    assert!(
        second.is_empty(),
        "history lines must be emitted exactly once"
    );

    let live = app.live_chat_lines();
    assert!(
        live.iter()
            .any(|line| line.to_string().contains("第二轮用户")),
        "latest user message should still appear in live viewport"
    );
    assert!(
        live.iter()
            .any(|line| line.to_string().contains("第二轮助手")),
        "latest assistant message should still appear in live viewport"
    );
}
