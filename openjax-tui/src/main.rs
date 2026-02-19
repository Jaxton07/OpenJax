use openjax_tui::app::App;
use openjax_tui::app_event::AppEvent;
use openjax_tui::tui::next_app_event;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::{self, Stdout};

fn setup_terminal() -> anyhow::Result<Terminal<CrosstermBackend<Stdout>>> {
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut terminal = setup_terminal()?;
    let mut app = App::default();

    loop {
        terminal.draw(|frame| app.render(frame))?;
        if app.should_quit() {
            break;
        }
        if let Some(event) = next_app_event()? {
            app.handle_event(event);
        } else {
            app.handle_event(AppEvent::Quit);
        }
    }

    Ok(())
}
