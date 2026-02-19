use openjax_tui::app::App;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

#[test]
fn render_smoke_fixed_size_terminal() {
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).expect("terminal should initialize");
    let app = App::default();

    terminal
        .draw(|frame| app.render(frame))
        .expect("render should succeed");
}
