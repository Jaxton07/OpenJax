use openjax_core::{Agent, Config, init_logger};
use openjax_protocol::Op;
use openjax_tui::app::App;
use openjax_tui::app_event::AppEvent;
use openjax_tui::tui::{AltScreenMode, enter_terminal_mode, next_app_event, restore_terminal_mode};
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
    init_logger();

    let alt_enabled = enter_terminal_mode(AltScreenMode::from_env())?;
    let _guard = scopeguard::guard(alt_enabled, |enabled| {
        let _ = restore_terminal_mode(enabled);
    });

    let config = Config::load();
    let mut agent = Agent::with_config(config);

    let mut terminal = setup_terminal()?;
    let mut app = App::default();
    app.state.push_system_message(format!(
        "TUI ready (model: {}, approval: {}, sandbox: {})",
        agent.model_backend_name(),
        agent.approval_policy_name(),
        agent.sandbox_mode_name()
    ));

    loop {
        terminal.draw(|frame| app.render(frame))?;
        if app.should_quit() {
            break;
        }
        if let Some(event) = next_app_event()? {
            match event {
                AppEvent::SubmitInput => {
                    let input = app.state.input.trim().to_string();
                    app.handle_event(AppEvent::SubmitInput);
                    if !input.is_empty() {
                        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                        let _ = agent.submit_with_sink(Op::UserTurn { input }, tx).await;
                        while let Ok(core_event) = rx.try_recv() {
                            app.handle_event(AppEvent::CoreEvent(core_event));
                        }
                    }
                }
                other => app.handle_event(other),
            }
        } else {
            app.handle_event(AppEvent::Quit);
        }
    }

    for event in agent.submit(Op::Shutdown).await {
        app.handle_event(AppEvent::CoreEvent(event));
    }

    Ok(())
}
