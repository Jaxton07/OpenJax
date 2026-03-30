//! Turn worker orchestration for gateway sessions.

use std::sync::Arc;

use openjax_policy::overlay::SessionOverlay;
use openjax_policy::runtime::PolicyRuntime;
use openjax_policy::store::PolicyStore;
use serde_json::json;
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::warn;

use crate::error::ApiError;

use super::config::gateway_policy_state;
use super::core_projection::{first_turn_id, map_core_event};
use super::events::AppState;
use super::runtime::{ApiTurnError, SessionRuntime, TurnStatus};
use super::append_then_publish;

pub async fn run_turn_task(
    app_state: AppState,
    session_runtime: Arc<Mutex<SessionRuntime>>,
    session_id: String,
    request_id: String,
    input: String,
    turn_id_tx: oneshot::Sender<Result<String, ApiError>>,
) {
    let (event_sink_tx, mut event_sink_rx) = mpsc::unbounded_channel();
    let (policy_level_override, overlay_rules, agent) = {
        let guard = session_runtime.lock().await;
        (
            guard.policy_level_override.clone(),
            guard.policy_overlay_rules.clone(),
            guard.agent.clone(),
        )
    };
    let policy_runtime = {
        let policy_state = gateway_policy_state(&app_state.store);
        let guard = match policy_state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(override_decision) = policy_level_override {
            // User explicitly set a policy level for this session.
            // Build an independent runtime so the gateway global default (Ask)
            // does not overwrite the user's choice on every turn.
            let store = PolicyStore::new(override_decision, vec![]);
            let fresh = PolicyRuntime::new(store);
            if !overlay_rules.is_empty() {
                fresh.set_session_overlay(&session_id, SessionOverlay::new(overlay_rules));
            }
            fresh
        } else {
            guard.runtime.clone()
        }
    };
    let session_id_for_core = session_id.clone();

    let submit_task = tokio::spawn(async move {
        let mut guard = agent.lock().await;
        guard.set_policy_runtime(Some(policy_runtime));
        guard.set_policy_session_id(Some(session_id_for_core));
        guard
            .submit_with_sink(openjax_protocol::Op::UserTurn { input }, event_sink_tx)
            .await
    });

    let should_abort_now = {
        let mut session = session_runtime.lock().await;
        session.current_turn_abort_handle = Some(submit_task.abort_handle());
        session.turn_submit_in_flight = false;
        session.turn_abort_requested
    };
    if should_abort_now {
        submit_task.abort();
    }

    let mut sent_turn_id = false;
    let mut last_known_public_turn_id: Option<String> = None;
    let mut pending_turn_id_tx = Some(turn_id_tx);
    while let Some(event) = event_sink_rx.recv().await {
        let mapped = {
            let mut session = session_runtime.lock().await;
            map_core_event(
                &app_state,
                &mut session,
                &session_id,
                &request_id,
                event,
                &mut pending_turn_id_tx,
            )
        };
        if let Some(turn_id) = mapped {
            sent_turn_id = true;
            last_known_public_turn_id = Some(turn_id);
        }
    }

    match submit_task.await {
        Ok(events) => {
            {
                let mut session = session_runtime.lock().await;
                session.current_turn_abort_handle = None;
                session.turn_submit_in_flight = false;
                session.turn_abort_requested = false;
            }
            if !sent_turn_id {
                let mut session = session_runtime.lock().await;
                if let Some(core_turn_id) = first_turn_id(&events) {
                    let public_turn_id = session.get_or_create_public_turn_id(core_turn_id);
                    if let Some(tx) = pending_turn_id_tx.take() {
                        let _ = tx.send(Ok(public_turn_id));
                    }
                } else if let Some(tx) = pending_turn_id_tx.take() {
                    let _ = tx.send(Err(ApiError::internal("failed to infer turn id")));
                }
            }
        }
        Err(join_error) => {
            {
                let mut session = session_runtime.lock().await;
                session.current_turn_abort_handle = None;
                session.turn_submit_in_flight = false;
                session.turn_abort_requested = false;
            }

            if join_error.is_cancelled() {
                let mut session = session_runtime.lock().await;
                let public_turn_id = last_known_public_turn_id.clone().or_else(|| {
                    session
                        .turns
                        .iter()
                        .find(|(_, turn)| turn.status == TurnStatus::Running)
                        .map(|(turn_id, _)| turn_id.clone())
                });
                if let Some(turn_id) = public_turn_id.as_ref()
                    && let Some(turn) = session.turns.get_mut(turn_id)
                    && matches!(turn.status, TurnStatus::Queued | TurnStatus::Running)
                {
                    turn.status = TurnStatus::Failed;
                    turn.error = Some(ApiTurnError {
                        code: "TURN_ABORTED".to_string(),
                        message: "turn aborted by user".to_string(),
                        retryable: false,
                        details: json!({ "reason": "user_abort" }),
                    });
                }
                let envelope = session.create_gateway_event(
                    &request_id,
                    &session_id,
                    public_turn_id.clone(),
                    "turn_interrupted",
                    json!({ "reason": "user_abort" }),
                    Some("synthetic"),
                );
                if let Err(err) = append_then_publish(&app_state, &mut session, envelope.clone()) {
                    warn!(
                        session_id = %envelope.session_id,
                        event_seq = envelope.event_seq,
                        event_type = %envelope.event_type,
                        error = %err.message,
                        "failed to append abort event before publish"
                    );
                }
            }

            if let Some(tx) = pending_turn_id_tx.take() {
                let err = if join_error.is_cancelled() {
                    ApiError::conflict("turn aborted by user", json!({ "reason": "user_abort" }))
                } else {
                    ApiError::upstream_unavailable("core execution task failed", json!({}))
                };
                let _ = tx.send(Err(err));
            }
        }
    }
}
