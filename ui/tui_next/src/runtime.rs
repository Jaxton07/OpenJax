use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use crossterm::event::{self, DisableBracketedPaste, EnableBracketedPaste};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use openjax_core::{Agent, Config, init_logger};
use openjax_protocol::Op;
use scopeguard::guard;
use tokio::sync::Mutex;

use crate::app::{App, SubmitAction};
use crate::approval::TuiApprovalHandler;
use crate::input::{InputAction, map_event};
use crate::tui::Tui;

pub async fn run() -> anyhow::Result<()> {
    init_logger();
    enable_raw_mode().context("failed to enable raw mode")?;
    execute!(std::io::stdout(), EnableBracketedPaste).ok();
    let _raw_guard = guard((), |_| {
        let _ = execute!(std::io::stdout(), DisableBracketedPaste);
        let _ = disable_raw_mode();
    });

    let config = Config::load();
    let mut agent = Agent::with_config(config);
    let approval_handler = Arc::new(TuiApprovalHandler::new());
    agent.set_approval_handler(approval_handler.clone());
    let agent = Arc::new(Mutex::new(agent));

    let mut app = App::default();
    app.initialize_banner_once();
    {
        let guard = agent.lock().await;
        app.set_runtime_info(
            guard.model_backend_name().to_string(),
            guard.approval_policy_name().to_string(),
            guard.sandbox_mode_name().to_string(),
        );
    }
    let mut tui = Tui::new()?;

    let mut turn_task: Option<tokio::task::JoinHandle<()>> = None;
    let mut core_event_rx: Option<tokio::sync::mpsc::UnboundedReceiver<openjax_protocol::Event>> =
        None;
    loop {
        while let Some(request) = approval_handler.pop_request().await {
            app.apply_core_event(openjax_protocol::Event::ApprovalRequested {
                turn_id: 0,
                request_id: request.request_id,
                target: request.target,
                reason: request.reason,
            });
        }

        if let Some(rx) = core_event_rx.as_mut() {
            while let Ok(event) = rx.try_recv() {
                app.apply_core_event(event);
            }
        }

        if turn_task.as_ref().is_some_and(|task| task.is_finished()) {
            if let Some(task) = turn_task.take() {
                let _ = task.await;
            }
            if let Some(mut rx) = core_event_rx.take() {
                while let Ok(event) = rx.try_recv() {
                    app.apply_core_event(event);
                }
            }
        }

        let viewport = tui.viewport_size();
        let term_width = viewport.width.max(8);
        let desired = app.desired_height(term_width);
        let cells = app.drain_history_cells();
        tui.queue_history_cells(cells);
        tui.draw(
            desired,
            app.input_line(),
            app.input_cursor_offset(term_width),
            app.approval_panel_lines(),
            app.footer_text(),
            |area, buf| app.render_live(area, buf),
        )?;

        if !event::poll(Duration::from_millis(40))? {
            continue;
        }
        let evt = event::read()?;
        match map_event(evt) {
            InputAction::Quit => break,
            InputAction::Submit => {
                if turn_task.is_some() && app.state.pending_approval.is_none() {
                    app.set_live_status("Busy: previous turn still running");
                    continue;
                }
                if let Some(action) = app.submit_input() {
                    match action {
                        SubmitAction::UserTurn { input } => {
                            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                            let agent = Arc::clone(&agent);
                            turn_task = Some(tokio::spawn(async move {
                                let mut guard = agent.lock().await;
                                let _ = guard.submit_with_sink(Op::UserTurn { input }, tx).await;
                            }));
                            core_event_rx = Some(rx);
                        }
                        SubmitAction::ApprovalDecision {
                            request_id,
                            approved,
                        } => {
                            let _ = approval_handler.resolve(&request_id, approved).await;
                        }
                    }
                }
            }
            InputAction::Backspace => app.backspace(),
            InputAction::MoveUp => app.move_approval_selection(-1),
            InputAction::MoveDown => app.move_approval_selection(1),
            InputAction::Append(text) => app.append_input(&text),
            InputAction::Clear => app.clear(),
            InputAction::None => {}
        }
    }

    let mut guard = agent.lock().await;
    let events = guard.submit(Op::Shutdown).await;
    for event in events {
        app.apply_core_event(event);
    }
    Ok(())
}
