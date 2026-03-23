use std::sync::Arc;
use std::time::Instant;

use openjax_core::Agent;
use openjax_protocol::Event;
use openjax_protocol::Op;
use tokio::sync::Mutex;
use tracing::info;

use crate::app::{App, SubmitAction};
use crate::approval::TuiApprovalHandler;
use crate::state::ApprovalSelection;
use crate::tui::{DrawRequest, Tui};

// Two-phase approval pattern:
// Phase 1 — TuiApprovalHandler queues a minimal ApprovalRequest (from core's internal
//   approval channel). We emit an ApprovalRequested event with empty optional fields
//   (tool_name, risk_tags, sandbox_backend, etc.) to show the approval panel immediately.
// Phase 2 — The gateway also forwards a richer ApprovalRequested event containing full
//   metadata. reducer.rs deduplicates by request_id and merges the richer fields into
//   the existing PendingApproval without creating a second state transition
//   (see reducer.rs apply_core_event, dedup block).
// This design lets the UI appear instantly while richer metadata arrives asynchronously.
pub(crate) async fn drain_approval_requests(app: &mut App, approval_handler: &TuiApprovalHandler) {
    let mut drained = 0usize;
    while let Some(request) = approval_handler.pop_request().await {
        drained += 1;
        info!(
            request_id = %request.request_id,
            reason = %request.reason,
            "tui approval request drained"
        );
        app.apply_core_event(openjax_protocol::Event::ApprovalRequested {
            turn_id: 0,
            request_id: request.request_id,
            target: request.target,
            reason: request.reason,
            tool_name: None,
            command_preview: None,
            risk_tags: Vec::new(),
            sandbox_backend: None,
            degrade_reason: None,
            policy_version: None,
            matched_rule_id: None,
        });
    }
    if drained > 0 {
        info!(count = drained, "tui approval requests drained");
    }
}

pub(crate) fn drain_core_events(
    app: &mut App,
    core_event_rx: &mut Option<tokio::sync::mpsc::UnboundedReceiver<openjax_protocol::Event>>,
) {
    if let Some(rx) = core_event_rx.as_mut() {
        let mut drained = 0usize;
        while let Ok(event) = rx.try_recv() {
            drained += 1;
            log_core_event(&event);
            app.apply_core_event(event);
        }
        if drained > 0 {
            info!(count = drained, "tui core events drained");
        }
    }
}

pub(crate) async fn drain_finished_turn_task(
    app: &mut App,
    turn_task: &mut Option<tokio::task::JoinHandle<()>>,
    core_event_rx: &mut Option<tokio::sync::mpsc::UnboundedReceiver<openjax_protocol::Event>>,
) {
    if turn_task.as_ref().is_some_and(|task| task.is_finished()) {
        if let Some(task) = turn_task.take() {
            let _ = task.await;
        }
        if let Some(mut rx) = core_event_rx.take() {
            let mut drained = 0usize;
            while let Ok(event) = rx.try_recv() {
                drained += 1;
                log_core_event(&event);
                app.apply_core_event(event);
            }
            if drained > 0 {
                info!(
                    count = drained,
                    "tui core events drained after turn finished"
                );
            }
        }
    }
}

pub(crate) fn render_once(app: &mut App, tui: &mut Tui) -> anyhow::Result<()> {
    let viewport = tui.viewport_size();
    let term_width = viewport.width.max(8);
    let desired = app.desired_height(term_width);
    let bottom_layout = app.bottom_layout(term_width);
    let status_line = app.status_bar_line(Instant::now(), term_width, true);
    let transient_panel = app.transient_panel();
    let reset_viewport = app.take_viewport_reset_requested();
    let cells = app.drain_history_cells();
    if !cells.is_empty() {
        info!(count = cells.len(), "tui rendering history cells");
    }
    tui.queue_history_cells(cells);
    tui.draw(
        DrawRequest {
            desired_height: desired,
            bottom_layout,
            reset_sticky_height: reset_viewport,
            status_line,
            input_line: app.input_line(),
            input_cursor: app.input_cursor_offset(term_width),
            transient_panel,
            footer_text: app.footer_text(),
        },
        |area, buf| app.render_live(area, buf),
    )?;
    Ok(())
}

fn log_core_event(event: &Event) {
    match event {
        Event::ToolCallStarted {
            turn_id,
            tool_name,
            target,
            ..
        } => info!(
            turn_id = *turn_id,
            tool_name = %tool_name,
            target = %target.as_deref().unwrap_or(""),
            "tui core event ToolCallStarted"
        ),
        Event::ToolCallCompleted {
            turn_id,
            tool_name,
            ok,
            ..
        } => info!(
            turn_id = *turn_id,
            tool_name = %tool_name,
            ok = *ok,
            "tui core event ToolCallCompleted"
        ),
        Event::ApprovalRequested {
            turn_id,
            request_id,
            tool_name,
            reason,
            ..
        } => info!(
            turn_id = *turn_id,
            request_id = %request_id,
            tool_name = %tool_name.as_deref().unwrap_or(""),
            reason = %reason,
            "tui core event ApprovalRequested"
        ),
        Event::ApprovalResolved {
            turn_id,
            request_id,
            approved,
        } => info!(
            turn_id = *turn_id,
            request_id = %request_id,
            approved = *approved,
            "tui core event ApprovalResolved"
        ),
        Event::TurnStarted { turn_id } => info!(turn_id = *turn_id, "tui core event TurnStarted"),
        Event::TurnCompleted { turn_id } => {
            info!(turn_id = *turn_id, "tui core event TurnCompleted")
        }
        _ => {}
    }
}

pub(crate) async fn handle_submit_action(
    app: &mut App,
    action: SubmitAction,
    agent: Arc<Mutex<Agent>>,
    approval_handler: Arc<TuiApprovalHandler>,
    turn_task: &mut Option<tokio::task::JoinHandle<()>>,
    core_event_rx: &mut Option<tokio::sync::mpsc::UnboundedReceiver<openjax_protocol::Event>>,
) {
    match action {
        SubmitAction::UserTurn { input } => {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            turn_task.replace(tokio::spawn(async move {
                let mut guard = agent.lock().await;
                let _ = guard.submit_with_sink(Op::UserTurn { input }, tx).await;
            }));
            core_event_rx.replace(rx);
        }
        SubmitAction::ApprovalDecision {
            request_id,
            approved,
        } => {
            let pending = app.state.pending_approval.clone();
            let approval_selection = app.state.approval_selection;
            let resolved = approval_handler.resolve(&request_id, approved).await;
            if resolved {
                app.state.pending_approval = None;
                app.state.approval_selection = ApprovalSelection::Approve;
                app.state.live_messages.clear();
            } else {
                if app.state.pending_approval.is_none() {
                    app.state.pending_approval = pending;
                    app.state.approval_selection = approval_selection;
                }
                app.set_live_status("Approval resolve failed; request remains pending");
            }
        }
        SubmitAction::CompactSession => {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            turn_task.replace(tokio::spawn(async move {
                let mut guard = agent.lock().await;
                let mut events = Vec::new();
                guard.compact(&mut events).await;
                for event in events {
                    let _ = tx.send(event);
                }
            }));
            core_event_rx.replace(rx);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Instant;

    use super::handle_submit_action;
    use crate::app::{App, SubmitAction};
    use crate::approval::TuiApprovalHandler;
    use crate::state::{ApprovalSelection, PendingApproval};
    use openjax_core::{Agent, Config};
    use tokio::sync::Mutex;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn drain_core_events_consumes_receiver_queue() {
        let mut app = App::default();
        let (tx, rx) = mpsc::unbounded_channel();
        tx.send(openjax_protocol::Event::TurnStarted { turn_id: 5 })
            .expect("send should work");
        let mut opt_rx = Some(rx);

        super::drain_core_events(&mut app, &mut opt_rx);

        assert_eq!(app.state.active_turn_id, Some(5));
    }

    #[tokio::test]
    async fn m10_approval_panel_navigation_resolve_failure_keeps_pending() {
        let mut app = App::default();
        app.state.pending_approval = Some(PendingApproval {
            request_id: "req-1".to_string(),
            target: "修改文件 test.txt".to_string(),
            reason: "需要写入文件".to_string(),
            tool_name: Some("shell".to_string()),
            command_preview: Some("echo hi > test.txt".to_string()),
            risk_tags: vec!["write".to_string()],
            sandbox_backend: Some("linux_native".to_string()),
            degrade_reason: None,
            requested_at: Instant::now(),
            timeout_ms: 300_000,
        });
        app.state.approval_selection = ApprovalSelection::Deny;

        let agent = Arc::new(Mutex::new(Agent::with_config(Config::default())));
        let approval_handler = Arc::new(TuiApprovalHandler::new());
        let mut turn_task = None;
        let mut core_event_rx = None;

        handle_submit_action(
            &mut app,
            SubmitAction::ApprovalDecision {
                request_id: "req-1".to_string(),
                approved: true,
            },
            agent,
            approval_handler,
            &mut turn_task,
            &mut core_event_rx,
        )
        .await;

        let pending = app
            .state
            .pending_approval
            .as_ref()
            .expect("approval should remain pending after resolve failure");
        assert_eq!(pending.request_id, "req-1");
        assert_eq!(app.state.approval_selection, ApprovalSelection::Deny);
        let status = app
            .state
            .live_messages
            .first()
            .expect("failure should surface a live status");
        assert_eq!(status.role, "status");
        assert!(
            status.content.contains("failed"),
            "failure should not look like a successful approval submit",
        );
    }
}
