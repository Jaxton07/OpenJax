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
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

const STALL_WARN_INTERVAL: Duration = Duration::from_secs(15);

fn setup_terminal() -> anyhow::Result<Terminal<CrosstermBackend<Stdout>>> {
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logger();
    info!("tui main started");

    let alt_enabled = enter_terminal_mode(AltScreenMode::from_env())?;
    info!(alt_screen = alt_enabled, "terminal mode initialized");
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
        info!(
            model = agent_guard.model_backend_name(),
            approval = agent_guard.approval_policy_name(),
            sandbox = agent_guard.sandbox_mode_name(),
            "tui runtime context initialized"
        );
    }

    let mut turn_task: Option<tokio::task::JoinHandle<()>> = None;
    let mut core_event_rx: Option<tokio::sync::mpsc::UnboundedReceiver<openjax_protocol::Event>> =
        None;
    let mut turn_started_at: Option<Instant> = None;
    let mut last_core_event_at: Option<Instant> = None;
    let mut last_stall_log_at: Option<Instant> = None;

    loop {
        while let Some(request) = approval_handler.pop_request().await {
            info!(
                request_id = %request.request_id,
                target = %request.target,
                reason = %request.reason,
                "approval request surfaced to tui"
            );
            app.state.enqueue_approval_request(
                request.request_id,
                0,
                request.target,
                request.reason,
            );
        }

        if let Some(rx) = core_event_rx.as_mut() {
            while let Ok(core_event) = rx.try_recv() {
                debug!(
                    event = event_name(&core_event),
                    "core event received by tui"
                );
                last_core_event_at = Some(Instant::now());
                app.handle_event(AppEvent::CoreEvent(core_event));
            }
        }

        if turn_task.is_some() {
            let now = Instant::now();
            if let (Some(started), Some(last_event)) = (turn_started_at, last_core_event_at) {
                let idle = now.duration_since(last_event);
                if idle >= STALL_WARN_INTERVAL
                    && last_stall_log_at
                        .map(|at| now.duration_since(at) >= STALL_WARN_INTERVAL)
                        .unwrap_or(true)
                {
                    warn!(
                        turn_elapsed_ms = now.duration_since(started).as_millis(),
                        idle_without_core_event_ms = idle.as_millis(),
                        "turn still running without new core events"
                    );
                    last_stall_log_at = Some(now);
                }
            }
        }

        if turn_task.as_ref().is_some_and(|task| task.is_finished()) {
            if let Some(task) = turn_task.take() {
                match task.await {
                    Ok(()) => info!("turn task joined"),
                    Err(err) => warn!(error = %err, "turn task join failed"),
                }
            }
            core_event_rx = None;
            turn_started_at = None;
            last_core_event_at = None;
            last_stall_log_at = None;
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
                    if app.state.approval.overlay_visible {
                        app.handle_event(AppEvent::SubmitInput);
                        continue;
                    }
                    if turn_task.is_some() {
                        warn!("submit ignored: previous turn still running");
                        app.state
                            .push_system_message("busy: previous turn still running".to_string());
                        continue;
                    }
                    let input = app.state.input_state.buffer.trim().to_string();
                    let preview = summarize_input(&input, 120);
                    info!(
                        input_len = input.chars().count(),
                        input_preview = %preview,
                        "user submitted input from tui"
                    );
                    app.handle_event(AppEvent::SubmitInput);
                    if !input.is_empty() {
                        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                        let agent = Arc::clone(&agent);
                        let input_len = input.chars().count();
                        let input_preview = summarize_input(&input, 120);
                        turn_task = Some(tokio::spawn(async move {
                            let mut agent_guard = agent.lock().await;
                            info!(
                                input_len,
                                input_preview = %input_preview,
                                "turn task started"
                            );
                            let fallback_events = agent_guard
                                .submit_with_sink(Op::UserTurn { input }, tx)
                                .await;
                            debug!(
                                fallback_event_count = fallback_events.len(),
                                "submit_with_sink returned"
                            );
                        }));
                        core_event_rx = Some(rx);
                        let now = Instant::now();
                        turn_started_at = Some(now);
                        last_core_event_at = Some(now);
                        last_stall_log_at = None;
                        info!("turn task spawned");
                    }
                }
                other => app.handle_event(other),
            }
        }

        while let Some((request_id, approved)) = app.take_pending_approval_decision() {
            if approval_handler.resolve(&request_id, approved).await {
                app.state.push_system_message(format!(
                    "approval decision sent: {}",
                    if approved { "approved" } else { "rejected" }
                ));
                info!(
                    request_id = %request_id,
                    approved = approved,
                    "approval decision resolved from tui"
                );
            }
        }
    }

    let mut agent_guard = agent.lock().await;
    info!("sending shutdown op");
    for event in agent_guard.submit(Op::Shutdown).await {
        app.handle_event(AppEvent::CoreEvent(event));
    }
    info!("tui main shutdown complete");

    Ok(())
}

fn summarize_input(input: &str, max_chars: usize) -> String {
    if input.is_empty() {
        return "<empty>".to_string();
    }
    let mut out = String::new();
    for ch in input.chars().take(max_chars) {
        out.push(ch);
    }
    if input.chars().count() > max_chars {
        out.push_str("...");
    }
    out
}

fn event_name(event: &openjax_protocol::Event) -> &'static str {
    match event {
        openjax_protocol::Event::TurnStarted { .. } => "TurnStarted",
        openjax_protocol::Event::ToolCallStarted { .. } => "ToolCallStarted",
        openjax_protocol::Event::ToolCallCompleted { .. } => "ToolCallCompleted",
        openjax_protocol::Event::AssistantMessage { .. } => "AssistantMessage",
        openjax_protocol::Event::AssistantDelta { .. } => "AssistantDelta",
        openjax_protocol::Event::ApprovalRequested { .. } => "ApprovalRequested",
        openjax_protocol::Event::ApprovalResolved { .. } => "ApprovalResolved",
        openjax_protocol::Event::AgentSpawned { .. } => "AgentSpawned",
        openjax_protocol::Event::AgentStatusChanged { .. } => "AgentStatusChanged",
        openjax_protocol::Event::TurnCompleted { .. } => "TurnCompleted",
        openjax_protocol::Event::ShutdownComplete => "ShutdownComplete",
    }
}
