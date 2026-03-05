use std::sync::Arc;
use std::time::Duration;
use std::{io, io::Write};

use anyhow::Context;
use crossterm::cursor::MoveTo;
use crossterm::event::{self, DisableBracketedPaste, EnableBracketedPaste};
use crossterm::execute;
use crossterm::terminal::{Clear, ClearType};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use openjax_core::{Agent, Config, init_split_logger};
use openjax_protocol::Op;
use scopeguard::guard;
use tokio::sync::Mutex;
use tracing::info;

use crate::app::App;
use crate::approval::TuiApprovalHandler;
use crate::input::{InputAction, map_event};
use crate::runtime_loop::{
    drain_approval_requests, drain_core_events, drain_finished_turn_task, handle_submit_action,
    render_once,
};
use crate::tui::Tui;

pub async fn run() -> anyhow::Result<()> {
    init_split_logger("openjax.log", "openjax_tui.log");
    info!("tui runtime starting");
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
    info!("tui initialized");
    let mut turn_task: Option<tokio::task::JoinHandle<()>> = None;
    let mut core_event_rx: Option<tokio::sync::mpsc::UnboundedReceiver<openjax_protocol::Event>> =
        None;

    loop {
        drain_approval_requests(&mut app, &approval_handler).await;
        drain_core_events(&mut app, &mut core_event_rx);
        drain_finished_turn_task(&mut app, &mut turn_task, &mut core_event_rx).await;
        render_once(&mut app, &mut tui)?;

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
                    handle_submit_action(
                        &mut app,
                        action,
                        Arc::clone(&agent),
                        Arc::clone(&approval_handler),
                        &mut turn_task,
                        &mut core_event_rx,
                    )
                    .await;
                }
            }
            InputAction::Backspace => app.backspace(),
            InputAction::MoveLeft => app.move_cursor_left(),
            InputAction::MoveRight => app.move_cursor_right(),
            InputAction::MoveUp => {
                if app.state.pending_approval.is_some() {
                    app.move_approval_selection(-1);
                } else {
                    app.history_prev();
                }
            }
            InputAction::MoveDown => {
                if app.state.pending_approval.is_some() {
                    app.move_approval_selection(1);
                } else {
                    app.history_next();
                }
            }
            InputAction::Append(text) => app.append_input(&text),
            InputAction::Clear => app.clear(),
            InputAction::None => {}
        }
    }

    let mut guard = agent.lock().await;
    info!("tui shutting down");
    let events = guard.submit(Op::Shutdown).await;
    for event in events {
        app.apply_core_event(event);
    }
    // Move cursor off the input/footer area so shell prompt lands on a clean line.
    let area = tui.viewport_size();
    let footer_y = area.bottom().saturating_sub(1);
    let mut stdout = io::stdout();
    let _ = execute!(stdout, MoveTo(0, footer_y), Clear(ClearType::CurrentLine));
    let _ = write!(stdout, "\r\n");
    let _ = stdout.flush();
    Ok(())
}
