use std::sync::OnceLock;
use std::time::Duration;

use axum::Json;
use axum::extract::{Extension, Path, State};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::oneshot;
use tokio::time::timeout;
use tracing::info;
use uuid::Uuid;

use crate::error::{ApiError, now_rfc3339};
use crate::event_mapper;
use crate::handlers::stream::publish_event_for_session;
use crate::middleware::RequestContext;
use crate::state::{AppState, SessionStatus, TurnRuntime, TurnStatus, run_turn_task};

static STREAM_DEBUG_ENABLED: OnceLock<bool> = OnceLock::new();

fn gateway_stream_debug_enabled() -> bool {
    *STREAM_DEBUG_ENABLED.get_or_init(|| {
        std::env::var("OPENJAX_GATEWAY_STREAM_DEBUG")
            .ok()
            .map(|value| {
                let normalized = value.trim().to_ascii_lowercase();
                !(normalized == "0"
                    || normalized == "off"
                    || normalized == "false"
                    || normalized == "disabled")
            })
            .unwrap_or(false)
    })
}

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct CreateSessionResponse {
    request_id: String,
    session_id: String,
    timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct SubmitTurnResponse {
    request_id: String,
    session_id: String,
    turn_id: String,
    timestamp: String,
}

#[derive(Debug, Deserialize)]
pub struct SubmitTurnRequest {
    input: String,
}

#[derive(Debug, Serialize)]
pub struct TurnResponse {
    request_id: String,
    session_id: String,
    turn_id: String,
    status: TurnStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    assistant_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<crate::state::ApiTurnError>,
    timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct ApprovalResolveResponse {
    request_id: String,
    session_id: String,
    approval_id: String,
    status: &'static str,
    timestamp: String,
}

#[derive(Debug, Deserialize)]
pub struct ResolveApprovalRequest {
    approved: bool,
    #[allow(dead_code)]
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SessionActionResponse {
    request_id: String,
    session_id: String,
    status: &'static str,
    timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct SessionSummary {
    session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct SessionListResponse {
    request_id: String,
    sessions: Vec<SessionSummary>,
    timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct SessionMessageItem {
    message_id: String,
    session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    turn_id: Option<String>,
    role: String,
    content: String,
    sequence: i64,
    created_at: String,
}

#[derive(Debug, Serialize)]
pub struct SessionMessagesResponse {
    request_id: String,
    session_id: String,
    messages: Vec<SessionMessageItem>,
    timestamp: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub async fn create_session(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<CreateSessionResponse>, ApiError> {
    let session_id = state.create_session().await?;
    Ok(Json(CreateSessionResponse {
        request_id: ctx.request_id,
        session_id,
        timestamp: now_rfc3339(),
    }))
}

pub async fn list_sessions(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<SessionListResponse>, ApiError> {
    let sessions = state
        .list_persisted_sessions()?
        .into_iter()
        .map(|item| SessionSummary {
            session_id: item.session_id,
            title: item.title,
            created_at: item.created_at,
            updated_at: item.updated_at,
        })
        .collect::<Vec<SessionSummary>>();
    Ok(Json(SessionListResponse {
        request_id: ctx.request_id,
        sessions,
        timestamp: now_rfc3339(),
    }))
}

pub async fn list_session_messages(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<SessionMessagesResponse>, ApiError> {
    let messages = state
        .list_session_messages(&session_id)?
        .into_iter()
        .map(|item| SessionMessageItem {
            message_id: item.message_id,
            session_id: item.session_id,
            turn_id: item.turn_id,
            role: item.role,
            content: item.content,
            sequence: item.sequence,
            created_at: item.created_at,
        })
        .collect::<Vec<SessionMessageItem>>();
    Ok(Json(SessionMessagesResponse {
        request_id: ctx.request_id,
        session_id,
        messages,
        timestamp: now_rfc3339(),
    }))
}

pub async fn submit_turn(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<SubmitTurnRequest>,
) -> Result<Json<SubmitTurnResponse>, ApiError> {
    let session_runtime = state.get_session(&session_id).await?;
    let input = payload.input.trim().to_string();
    {
        let mut session = session_runtime.lock().await;
        if matches!(
            session.status,
            SessionStatus::Closed | SessionStatus::Closing
        ) {
            return Err(ApiError::conflict(
                "session is not active",
                json!({ "session_id": session_id }),
            ));
        }
        if !input.is_empty() {
            state.append_message(&session_id, None, "user", &input)?;
            let user_event = session.create_gateway_event(
                &ctx.request_id,
                &session_id,
                None,
                "user_message",
                json!({ "content": input }),
                Some("synthetic"),
            );
            publish_event_for_session(&state, &mut session, user_event);
        }
    }

    let (turn_id_tx, turn_id_rx) = oneshot::channel();
    tokio::spawn(run_turn_task(
        state.clone(),
        session_runtime,
        session_id.clone(),
        ctx.request_id.clone(),
        input,
        turn_id_tx,
    ));

    let turn_id = match timeout(Duration::from_secs(20), turn_id_rx).await {
        Ok(Ok(Ok(turn_id))) => turn_id,
        Ok(Ok(Err(err))) => return Err(err),
        Ok(Err(_)) => {
            return Err(ApiError::upstream_unavailable(
                "turn worker stopped unexpectedly",
                json!({ "session_id": session_id }),
            ));
        }
        Err(_) => {
            return Err(ApiError::timeout(
                "timed out waiting for turn id",
                json!({ "session_id": session_id }),
            ));
        }
    };

    Ok(Json(SubmitTurnResponse {
        request_id: ctx.request_id,
        session_id,
        turn_id,
        timestamp: now_rfc3339(),
    }))
}

pub async fn get_turn(
    State(state): State<AppState>,
    Path((session_id, turn_id)): Path<(String, String)>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<TurnResponse>, ApiError> {
    let session_runtime = state.get_session(&session_id).await?;
    let session = session_runtime.lock().await;
    let turn = session.turns.get(&turn_id).ok_or_else(|| {
        ApiError::not_found(
            "turn not found",
            json!({ "session_id": session_id, "turn_id": turn_id }),
        )
    })?;
    if gateway_stream_debug_enabled() {
        info!(
            request_id = %ctx.request_id,
            session_id = %session_id,
            turn_id = %turn_id,
            status = ?turn.status,
            assistant_message_len = ?turn.assistant_message.as_ref().map(|msg| msg.len()),
            has_error = turn.error.is_some(),
            "stream_debug.get_turn_response"
        );
    }
    Ok(Json(TurnResponse {
        request_id: ctx.request_id,
        session_id,
        turn_id,
        status: turn.status,
        assistant_message: turn.assistant_message.clone(),
        error: turn.error.clone(),
        timestamp: now_rfc3339(),
    }))
}

pub async fn resolve_approval(
    State(state): State<AppState>,
    Path((session_id, approval_action)): Path<(String, String)>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<ResolveApprovalRequest>,
) -> Result<Json<ApprovalResolveResponse>, ApiError> {
    let approval_action = approval_action.trim_start_matches('/');
    let approval_id = approval_action
        .strip_suffix(":resolve")
        .ok_or_else(|| {
            ApiError::not_found("invalid approval route", json!({ "path": approval_action }))
        })?
        .to_string();

    let session_runtime = state.get_session(&session_id).await?;
    let approval_handler = {
        let session = session_runtime.lock().await;
        if session.resolved_approvals.contains(&approval_id) {
            return Err(ApiError::conflict(
                "approval already resolved",
                json!({ "approval_id": approval_id }),
            ));
        }
        session.approval_handler.clone()
    };

    if !approval_handler
        .resolve(&approval_id, payload.approved)
        .await
    {
        return Err(ApiError::not_found(
            "approval not found",
            json!({ "approval_id": approval_id }),
        ));
    }

    let mut session = session_runtime.lock().await;
    session.resolved_approvals.insert(approval_id.clone());
    info!(
        request_id = %ctx.request_id,
        session_id = %session_id,
        approval_id = %approval_id,
        approved = payload.approved,
        actor = %ctx.actor,
        "approval resolved"
    );

    Ok(Json(ApprovalResolveResponse {
        request_id: ctx.request_id,
        session_id,
        approval_id,
        status: "resolved",
        timestamp: now_rfc3339(),
    }))
}

pub async fn session_action(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<SessionActionResponse>, ApiError> {
    let (session_id, action) = parse_session_action(&session_id)?;
    let normalized = normalize_session_action(action);
    let session_runtime = state.get_session(session_id).await?;
    if normalized == "compact" {
        let turn_id = format!("turn_action_{}", Uuid::new_v4().simple());
        handle_compact_action(
            &state,
            &session_runtime,
            &ctx.request_id,
            session_id,
            &turn_id,
        )
        .await?;
        return Ok(Json(SessionActionResponse {
            request_id: ctx.request_id,
            session_id: session_id.to_string(),
            status: "compacted",
            timestamp: now_rfc3339(),
        }));
    }
    if normalized != "clear" {
        return Err(ApiError::not_found(
            "invalid session action route",
            json!({ "action": action }),
        ));
    }
    clear_runtime(&state, &session_runtime).await;
    Ok(Json(SessionActionResponse {
        request_id: ctx.request_id,
        session_id: session_id.to_string(),
        status: "cleared",
        timestamp: now_rfc3339(),
    }))
}

pub async fn shutdown_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<SessionActionResponse>, ApiError> {
    let session_runtime = state.get_session(&session_id).await?;

    {
        let mut session = session_runtime.lock().await;
        let event = session.create_gateway_event(
            &ctx.request_id,
            &session_id,
            None,
            "session_shutdown",
            json!({ "reason": "shutdown_requested" }),
            None,
        );
        publish_event_for_session(&state, &mut session, event);
        session.status = SessionStatus::Closing;
        let agent = session.agent.clone();
        drop(session);
        let mut agent = agent.lock().await;
        let _ = agent.submit(openjax_protocol::Op::Shutdown).await;
        let mut session = session_runtime.lock().await;
        session.status = SessionStatus::Closed;
    }
    state.delete_session(&session_id).await?;

    Ok(Json(SessionActionResponse {
        request_id: ctx.request_id,
        session_id,
        status: "shutdown",
        timestamp: now_rfc3339(),
    }))
}

// ---------------------------------------------------------------------------
// Compact / Clear helpers
// ---------------------------------------------------------------------------

pub(crate) async fn handle_compact_action(
    state: &AppState,
    session_runtime: &tokio::sync::Mutex<crate::state::SessionRuntime>,
    request_id: &str,
    session_id: &str,
    turn_id: &str,
) -> Result<(), ApiError> {
    let agent = {
        let session = session_runtime.lock().await;
        session.agent.clone()
    };

    let mut events = Vec::new();
    {
        let mut agent = agent.lock().await;
        agent.compact(&mut events).await;
    }

    let mut session = session_runtime.lock().await;

    session.turns.insert(
        turn_id.to_string(),
        TurnRuntime {
            status: TurnStatus::Completed,
            assistant_message: Some("context compacted".to_string()),
            error: None,
        },
    );

    let started = session.create_gateway_event(
        request_id,
        session_id,
        Some(turn_id.to_string()),
        "turn_started",
        json!({}),
        None,
    );
    publish_event_for_session(state, &mut session, started);

    // Map and publish core events emitted by compact (e.g. ContextCompacted)
    for event in &events {
        if let Some(mapping) = event_mapper::map_core_event_payload(event) {
            let gateway_event = session.create_gateway_event(
                request_id,
                session_id,
                Some(turn_id.to_string()),
                mapping.event_type,
                mapping.payload,
                mapping.stream_source,
            );
            publish_event_for_session(state, &mut session, gateway_event);
        }
    }

    let response_event = session.create_gateway_event(
        request_id,
        session_id,
        Some(turn_id.to_string()),
        "response_completed",
        json!({ "content": "context compacted", "stream_source": "synthetic" }),
        Some("synthetic"),
    );
    let completed = session.create_gateway_event(
        request_id,
        session_id,
        Some(turn_id.to_string()),
        "turn_completed",
        json!({}),
        None,
    );
    publish_event_for_session(state, &mut session, response_event);
    publish_event_for_session(state, &mut session, completed);

    state.append_message(session_id, Some(turn_id), "assistant", "context compacted")?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub fn normalize_session_action(action: &str) -> String {
    action
        .trim_start_matches('/')
        .trim_start_matches(':')
        .to_string()
}

pub fn parse_session_action(session_action: &str) -> Result<(&str, &str), ApiError> {
    session_action.split_once(':').ok_or_else(|| {
        ApiError::not_found(
            "invalid session action route",
            json!({ "session_action": session_action }),
        )
    })
}

pub async fn clear_runtime(
    state: &AppState,
    session_runtime: &tokio::sync::Mutex<crate::state::SessionRuntime>,
) {
    let config = state.runtime_config();
    let mut session = session_runtime.lock().await;
    session.clear_context_with_config(config);
}

// ---------------------------------------------------------------------------
// Policy level
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SetPolicyLevelRequest {
    pub level: String,
}

#[derive(Debug, Serialize)]
pub struct SetPolicyLevelResponse {
    pub level: String,
}

/// PUT /api/v1/sessions/:session_id/policy
pub async fn set_policy_level(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(body): Json<SetPolicyLevelRequest>,
) -> Result<Json<SetPolicyLevelResponse>, ApiError> {
    let level = openjax_core::PolicyLevel::from_str(&body.level).ok_or_else(|| {
        ApiError::invalid_argument(
            format!(
                "'{}' is not a valid policy level; use permissive, standard, or strict",
                body.level
            ),
            json!({ "level": body.level }),
        )
    })?;

    // Take Arc to agent (releasing session map lock before locking agent)
    let agent_arc = {
        let session_runtime = state.get_session(&session_id).await?;
        let session = session_runtime.lock().await;
        session.agent.clone()
    };
    agent_arc.lock().await.set_policy_level(level);

    Ok(Json(SetPolicyLevelResponse {
        level: level.as_str().to_string(),
    }))
}
