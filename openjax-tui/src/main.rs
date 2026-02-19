use openjax_core::{Agent, Config, init_logger};
use openjax_protocol::Op;
use openjax_tui::app::App;
use openjax_tui::app_event::AppEvent;
use openjax_tui::approval::TuiApprovalHandler;
use openjax_tui::tui::{AltScreenMode, enter_terminal_mode, next_app_event, restore_terminal_mode};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::{self, Stdout};
use std::sync::Arc;

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
    let approval_handler = Arc::new(TuiApprovalHandler::new());
    agent.set_approval_handler(approval_handler.clone());
    let agent = Arc::new(tokio::sync::Mutex::new(agent));

    let mut terminal = setup_terminal()?;
    let mut app = App::default();
    app.state.show_system_messages = std::env::var("OPENJAX_TUI_SHOW_SYSTEM_EVENTS")
        .ok()
        .is_some_and(|v| matches!(v.as_str(), "1" | "true" | "yes"));
    {
        let agent_guard = agent.lock().await;
        app.state.model_name = Some(agent_guard.model_backend_name().to_string());
        app.state.approval_policy = Some(agent_guard.approval_policy_name().to_string());
        app.state.sandbox_mode = Some(agent_guard.sandbox_mode_name().to_string());
    }

    let mut turn_task: Option<tokio::task::JoinHandle<()>> = None;
    let mut core_event_rx: Option<tokio::sync::mpsc::UnboundedReceiver<openjax_protocol::Event>> =
        None;

    loop {
        if let Some(rx) = core_event_rx.as_mut() {
            while let Ok(core_event) = rx.try_recv() {
                app.handle_event(AppEvent::CoreEvent(core_event));
            }
        }
        if turn_task.as_ref().is_some_and(|task| task.is_finished()) {
            if let Some(task) = turn_task.take() {
                let _ = task.await;
            }
            core_event_rx = None;
            app.state
                .push_system_message("turn task completed".to_string());
        }

        terminal.draw(|frame| app.render(frame))?;
        if app.should_quit() {
            break;
        }
        if let Some(event) = next_app_event()? {
            match event {
                AppEvent::SubmitInput => {
                    if turn_task.is_some() {
                        app.state
                            .push_system_message("busy: previous turn still running".to_string());
                        continue;
                    }
                    let input = app.state.input.trim().to_string();
                    app.handle_event(AppEvent::SubmitInput);
                    if !input.is_empty() {
                        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                        let agent = Arc::clone(&agent);
                        turn_task = Some(tokio::spawn(async move {
                            let mut agent_guard = agent.lock().await;
                            let _ = agent_guard
                                .submit_with_sink(Op::UserTurn { input }, tx)
                                .await;
                        }));
                        core_event_rx = Some(rx);
                    }
                }
                AppEvent::InputChar(ch) if ch == 'y' || ch == 'n' => {
                    if let Some(overlay) = &app.state.approval_overlay {
                        let approved = ch == 'y';
                        if approval_handler
                            .resolve(&overlay.request_id, approved)
                            .await
                        {
                            app.state.push_system_message(format!(
                                "approval decision sent: {}",
                                if approved { "approved" } else { "rejected" }
                            ));
                            continue;
                        }
                    }
                    app.handle_event(AppEvent::InputChar(ch));
                }
                other => app.handle_event(other),
            }
        }
    }

    let mut agent_guard = agent.lock().await;
    for event in agent_guard.submit(Op::Shutdown).await {
        app.handle_event(AppEvent::CoreEvent(event));
    }

    Ok(())
}
