use std::sync::Arc;
use std::time::Duration;
use std::{io, io::Write};

use anyhow::Context;
use crossterm::cursor::MoveTo;
use crossterm::event::{self, DisableBracketedPaste, EnableBracketedPaste};
use crossterm::execute;
use crossterm::terminal::{Clear, ClearType};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use openjax_core::{Agent, init_split_logger, load_runtime_config};
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

fn dismiss_overlay(
    app: &mut App,
    turn_task: &mut Option<tokio::task::JoinHandle<()>>,
    core_event_rx: &mut Option<tokio::sync::mpsc::UnboundedReceiver<openjax_protocol::Event>>,
) {
    if app.state.policy_picker.is_some() {
        app.dismiss_policy_picker();
    } else if app.is_slash_palette_active() {
        app.dismiss_slash_palette();
    } else if app.state.pending_approval.is_some() {
        app.defer_pending_approval();
    } else if turn_task.is_some() {
        abort_turn(app, turn_task, core_event_rx);
    } else {
        app.clear();
    }
}

fn abort_turn(
    app: &mut App,
    turn_task: &mut Option<tokio::task::JoinHandle<()>>,
    core_event_rx: &mut Option<tokio::sync::mpsc::UnboundedReceiver<openjax_protocol::Event>>,
) {
    if let Some(task) = turn_task.take() {
        task.abort();
    }
    core_event_rx.take();
    let turn_id = app.state.active_turn_id.unwrap_or(0);
    app.apply_core_event(openjax_protocol::Event::TurnCompleted { turn_id });
    app.set_live_status("已中断");
}

pub async fn run() -> anyhow::Result<()> {
    init_split_logger("openjax.log", "openjax_tui.log");
    info!("tui runtime starting");
    enable_raw_mode().context("failed to enable raw mode")?;
    execute!(std::io::stdout(), EnableBracketedPaste).ok();
    let _raw_guard = guard((), |_| {
        let _ = execute!(std::io::stdout(), DisableBracketedPaste);
        let _ = disable_raw_mode();
    });

    let config = load_runtime_config();
    let mut agent = Agent::with_config(config);
    let approval_handler = Arc::new(TuiApprovalHandler::new());
    agent.set_approval_handler(approval_handler.clone());
    let agent = Arc::new(Mutex::new(agent));
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    let mut app = App::default();
    {
        let guard = agent.lock().await;
        app.set_runtime_info(
            guard.model_backend_name().to_string(),
            guard.policy_default_decision_name().to_string(),
            guard.sandbox_mode_name().to_string(),
            cwd.as_path(),
        );
    }
    app.initialize_banner_once();

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
                // policy picker 确认（优先，不受 busy 状态阻断）
                if app.state.policy_picker.is_some() {
                    let idx = app.state.policy_picker.as_ref().unwrap().selected_index;
                    let levels = ["allow", "ask", "deny"];
                    if let Some(&level_str) = levels.get(idx) {
                        if let Ok(level) = level_str.parse::<openjax_core::PolicyLevel>() {
                            agent.lock().await.set_policy_level(level);
                        }
                        app.apply_policy_pick(level_str);
                    }
                    continue;
                }
                if turn_task.is_some() && app.state.pending_approval.is_none() {
                    app.set_live_status("Busy: previous turn still running");
                    continue;
                }
                if app.state.pending_approval.is_none()
                    && app.is_slash_palette_active()
                    && app.complete_slash_selection() != crate::app::SlashAcceptResult::None
                {
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
            InputAction::AcceptSuggestion => {
                if app.state.pending_approval.is_none() && app.is_slash_palette_active() {
                    let _ = app.complete_slash_selection();
                }
            }
            InputAction::Backspace => app.backspace(),
            InputAction::MoveLeft => app.move_cursor_left(),
            InputAction::MoveRight => app.move_cursor_right(),
            InputAction::MoveUp => {
                if app.state.policy_picker.is_some() {
                    app.move_policy_selection(-1);
                } else if app.state.pending_approval.is_some() {
                    app.move_approval_selection(-1);
                } else if app.is_slash_palette_active() {
                    app.move_slash_selection(-1);
                } else {
                    app.history_prev();
                }
            }
            InputAction::MoveDown => {
                if app.state.policy_picker.is_some() {
                    app.move_policy_selection(1);
                } else if app.state.pending_approval.is_some() {
                    app.move_approval_selection(1);
                } else if app.is_slash_palette_active() {
                    app.move_slash_selection(1);
                } else {
                    app.history_next();
                }
            }
            InputAction::Append(text) => app.append_input(&text),
            InputAction::DismissOverlay => {
                dismiss_overlay(&mut app, &mut turn_task, &mut core_event_rx)
            }
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

#[cfg(test)]
mod tests {
    use super::{abort_turn, dismiss_overlay};
    use crate::app::App;
    use crate::state::PendingApproval;

    #[test]
    fn dismiss_overlay_with_pending_approval_defers_request() {
        let mut app = App::default();
        app.state.pending_approval = Some(PendingApproval {
            request_id: "req-1".to_string(),
            target: "target".to_string(),
            reason: "reason".to_string(),
            tool_name: Some("shell".to_string()),
            command_preview: Some("echo hi".to_string()),
            risk_tags: vec!["write".to_string()],
            sandbox_backend: Some("linux_native".to_string()),
            degrade_reason: None,
        });
        app.state.input = "tmp".to_string();
        app.state.input_cursor = 3;

        let mut turn_task: Option<tokio::task::JoinHandle<()>> = None;
        let mut core_event_rx: Option<
            tokio::sync::mpsc::UnboundedReceiver<openjax_protocol::Event>,
        > = None;
        dismiss_overlay(&mut app, &mut turn_task, &mut core_event_rx);

        assert!(app.state.pending_approval.is_some());
        assert!(app.state.input.is_empty());
        assert_eq!(app.state.input_cursor, 0);
        let status = app
            .state
            .live_messages
            .first()
            .expect("status should be visible");
        assert_eq!(status.role, "status");
        assert_eq!(
            status.content,
            "Approval pending: choose Approve or Deny when ready"
        );
    }

    #[tokio::test]
    async fn abort_turn_clears_task_and_sets_status() {
        let mut app = App::default();
        app.apply_core_event(openjax_protocol::Event::TurnStarted { turn_id: 42 });
        let mut turn_task: Option<tokio::task::JoinHandle<()>> = Some(tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_secs(9999)).await
        }));
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<openjax_protocol::Event>();
        drop(tx);
        let mut core_event_rx = Some(rx);

        abort_turn(&mut app, &mut turn_task, &mut core_event_rx);

        assert!(turn_task.is_none());
        assert!(core_event_rx.is_none());
        assert!(app.state.active_turn_id.is_none());
        let status = app
            .state
            .live_messages
            .first()
            .expect("status should exist");
        assert!(status.content.contains("已中断"));
    }
}
