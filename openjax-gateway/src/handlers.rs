use std::convert::Infallible;
use std::sync::OnceLock;
use std::time::Duration;

use async_stream::stream;
use axum::extract::{Extension, Path, Query, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::{Json, response::IntoResponse};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::oneshot;
use tokio::time::timeout;
use tracing::{info, warn};
use uuid::Uuid;

use crate::error::{ApiError, now_rfc3339};
use crate::middleware::RequestContext;
use crate::state::{
    ApiTurnError, AppState, SessionStatus, StreamEventEnvelope, TurnRuntime, TurnStatus,
    run_turn_task,
};

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

#[derive(Debug, Serialize)]
pub struct TurnResponse {
    request_id: String,
    session_id: String,
    turn_id: String,
    status: TurnStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    assistant_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ApiTurnError>,
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

#[derive(Debug, Serialize)]
pub struct SessionTimelineResponse {
    request_id: String,
    session_id: String,
    events: Vec<StreamEventEnvelope>,
    timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct ProviderItem {
    provider_id: String,
    provider_name: String,
    base_url: String,
    model_name: String,
    api_key_set: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct ProviderListResponse {
    request_id: String,
    providers: Vec<ProviderItem>,
    timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct ProviderMutationResponse {
    request_id: String,
    provider: ProviderItem,
    timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct ProviderDeleteResponse {
    request_id: String,
    provider_id: String,
    status: &'static str,
    timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct ActiveProviderItem {
    provider_id: String,
    model_name: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct ActiveProviderResponse {
    request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    active_provider: Option<ActiveProviderItem>,
    timestamp: String,
}

#[derive(Debug, Deserialize)]
pub struct SubmitTurnRequest {
    input: String,
}

#[derive(Debug, Deserialize)]
pub struct ResolveApprovalRequest {
    approved: bool,
    #[allow(dead_code)]
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EventsQuery {
    after_event_seq: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProviderRequest {
    provider_name: String,
    base_url: String,
    model_name: String,
    api_key: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProviderRequest {
    provider_name: String,
    base_url: String,
    model_name: String,
    api_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SetActiveProviderRequest {
    provider_id: String,
}

fn resolve_resume_seq(after_event_seq: Option<u64>, last_event_id: Option<&str>) -> Option<u64> {
    after_event_seq.or_else(|| last_event_id.and_then(|value| value.parse::<u64>().ok()))
}

pub async fn healthz() -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}

pub async fn readyz() -> impl IntoResponse {
    Json(json!({ "status": "ready" }))
}

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

pub async fn list_session_timeline(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Query(query): Query<EventsQuery>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<SessionTimelineResponse>, ApiError> {
    let events = state
        .list_session_events(&session_id, query.after_event_seq)?
        .into_iter()
        .map(|item| StreamEventEnvelope {
            request_id: format!("req_timeline_{}", item.id),
            session_id: item.session_id,
            turn_id: item.turn_id,
            event_seq: item.event_seq,
            turn_seq: item.turn_seq,
            timestamp: item.timestamp,
            event_type: item.event_type,
            stream_source: item.stream_source,
            payload: serde_json::from_str::<serde_json::Value>(&item.payload_json).unwrap_or_else(|_| json!({})),
        })
        .collect::<Vec<StreamEventEnvelope>>();
    Ok(Json(SessionTimelineResponse {
        request_id: ctx.request_id,
        session_id,
        events,
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

    if input == "/compact" {
        return Err(ApiError::not_implemented(
            "compact is not implemented yet",
            json!({ "session_id": session_id }),
        ));
    }
    if input == "/clear" {
        clear_runtime(&state, &session_runtime).await;
        let turn_id = format!("turn_cmd_{}", Uuid::new_v4().simple());
        let mut session = session_runtime.lock().await;
        session.turns.insert(
            turn_id.clone(),
            TurnRuntime {
                status: TurnStatus::Completed,
                assistant_message: Some("session cleared".to_string()),
                error: None,
            },
        );
        let started = session.create_gateway_event(
            &ctx.request_id,
            &session_id,
            Some(turn_id.clone()),
            "turn_started",
            json!({}),
            None,
        );
        let message = session.create_gateway_event(
            &ctx.request_id,
            &session_id,
            Some(turn_id.clone()),
            "response_started",
            json!({ "stream_source": "synthetic" }),
            Some("synthetic"),
        );
        let completed_response = session.create_gateway_event(
            &ctx.request_id,
            &session_id,
            Some(turn_id.clone()),
            "response_completed",
            json!({ "content": "session cleared", "stream_source": "synthetic" }),
            Some("synthetic"),
        );
        let completed = session.create_gateway_event(
            &ctx.request_id,
            &session_id,
            Some(turn_id.clone()),
            "turn_completed",
            json!({}),
            None,
        );
        publish_event_for_session(&state, &mut session, started);
        publish_event_for_session(&state, &mut session, message);
        publish_event_for_session(&state, &mut session, completed_response);
        publish_event_for_session(&state, &mut session, completed);
        state.append_message(&session_id, Some(&turn_id), "assistant", "session cleared")?;
        return Ok(Json(SubmitTurnResponse {
            request_id: ctx.request_id,
            session_id,
            turn_id,
            timestamp: now_rfc3339(),
        }));
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
    if normalized == "compact" {
        return Err(ApiError::not_implemented(
            "compact is not implemented yet",
            json!({ "session_id": session_id }),
        ));
    }
    if normalized != "clear" {
        return Err(ApiError::not_found(
            "invalid session action route",
            json!({ "action": action }),
        ));
    }
    let session_runtime = state.get_session(session_id).await?;
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
    state.remove_session(&session_id).await;

    Ok(Json(SessionActionResponse {
        request_id: ctx.request_id,
        session_id,
        status: "shutdown",
        timestamp: now_rfc3339(),
    }))
}

pub async fn stream_events(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Query(query): Query<EventsQuery>,
    headers: HeaderMap,
) -> Result<Sse<impl futures_util::Stream<Item = Result<SseEvent, Infallible>>>, ApiError> {
    let session_runtime = state.get_session(&session_id).await?;
    let (replay, mut rx) = {
        let session = session_runtime.lock().await;
        let last_event_id = headers
            .get("Last-Event-ID")
            .and_then(|value| value.to_str().ok());
        let after_event_seq = resolve_resume_seq(query.after_event_seq, last_event_id);
        (
            session.replay_from(after_event_seq)?,
            session.event_tx.subscribe(),
        )
    };

    let session_runtime_for_recovery = session_runtime.clone();
    let request_id = headers
        .get("X-Request-Id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("req_stream")
        .to_string();
    if gateway_stream_debug_enabled() {
        info!(
            request_id = %request_id,
            session_id = %session_id,
            after_event_seq = ?query.after_event_seq,
            replay_count = replay.len(),
            "stream_debug.stream_events_opened"
        );
    }
    let event_stream = stream! {
        let mut last_sent_event_seq = replay.last().map(|event| event.event_seq).unwrap_or(0);
        for event in replay {
            last_sent_event_seq = event.event_seq;
            yield Ok(to_sse_event(event));
        }
        loop {
            match rx.recv().await {
                Ok(event) => {
                    last_sent_event_seq = event.event_seq;
                    yield Ok(to_sse_event(event));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    let recovered = {
                        let session = session_runtime_for_recovery.lock().await;
                        session.replay_from(Some(last_sent_event_seq))
                    };
                    match recovered {
                        Ok(missed) if !missed.is_empty() => {
                            info!(
                                session_id = %session_id,
                                recovered_count = missed.len(),
                                last_sent_event_seq = last_sent_event_seq,
                                "sse_lagged_recovered"
                            );
                            for event in missed {
                                if event.event_seq <= last_sent_event_seq {
                                    continue;
                                }
                                last_sent_event_seq = event.event_seq;
                                yield Ok(to_sse_event(event));
                            }
                        }
                        _ => {
                            warn!(
                                session_id = %session_id,
                                last_sent_event_seq = last_sent_event_seq,
                                "sse_lagged_recovery_failed"
                            );
                            let recovery_error = StreamEventEnvelope {
                                request_id: request_id.clone(),
                                session_id: session_id.clone(),
                                turn_id: None,
                                event_seq: last_sent_event_seq.saturating_add(1),
                                turn_seq: 0,
                                timestamp: now_rfc3339(),
                                event_type: "response_error".to_string(),
                                stream_source: "replay".to_string(),
                                payload: json!({
                                    "code": "REPLAY_WINDOW_EXCEEDED",
                                    "message": "event replay window exceeded; reconnect required",
                                    "retryable": true
                                }),
                            };
                            yield Ok(to_sse_event(recovery_error));
                            break;
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    if gateway_stream_debug_enabled() {
                        info!(
                            request_id = %request_id,
                            session_id = %session_id,
                            last_sent_event_seq = last_sent_event_seq,
                            "stream_debug.stream_events_closed"
                        );
                    }
                    break;
                }
            }
        }
    };

    Ok(Sse::new(event_stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

pub async fn list_providers(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<ProviderListResponse>, ApiError> {
    let providers = state
        .list_providers()?
        .into_iter()
        .map(to_provider_item)
        .collect::<Vec<ProviderItem>>();
    Ok(Json(ProviderListResponse {
        request_id: ctx.request_id,
        providers,
        timestamp: now_rfc3339(),
    }))
}

pub async fn get_active_provider(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<ActiveProviderResponse>, ApiError> {
    let active_provider = state.get_active_provider()?.map(to_active_provider_item);
    Ok(Json(ActiveProviderResponse {
        request_id: ctx.request_id,
        active_provider,
        timestamp: now_rfc3339(),
    }))
}

pub async fn set_active_provider(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<SetActiveProviderRequest>,
) -> Result<Json<ActiveProviderResponse>, ApiError> {
    let provider_id = payload.provider_id.trim();
    if provider_id.is_empty() {
        return Err(ApiError::invalid_argument(
            "provider_id must not be empty",
            json!({}),
        ));
    }
    let selected = state.set_active_provider(provider_id)?.ok_or_else(|| {
        ApiError::not_found("provider not found", json!({ "provider_id": provider_id }))
    })?;
    Ok(Json(ActiveProviderResponse {
        request_id: ctx.request_id,
        active_provider: Some(to_active_provider_item(selected)),
        timestamp: now_rfc3339(),
    }))
}

pub async fn create_provider(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<CreateProviderRequest>,
) -> Result<Json<ProviderMutationResponse>, ApiError> {
    let provider_name = payload.provider_name.trim();
    let base_url = payload.base_url.trim();
    let model_name = payload.model_name.trim();
    let api_key = payload.api_key.trim();
    if provider_name.is_empty()
        || base_url.is_empty()
        || model_name.is_empty()
        || api_key.is_empty()
    {
        return Err(ApiError::invalid_argument(
            "provider fields must not be empty",
            json!({}),
        ));
    }
    let created = state.create_provider(provider_name, base_url, model_name, api_key)?;
    Ok(Json(ProviderMutationResponse {
        request_id: ctx.request_id,
        provider: to_provider_item(created),
        timestamp: now_rfc3339(),
    }))
}

pub async fn update_provider(
    State(state): State<AppState>,
    Path(provider_id): Path<String>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<UpdateProviderRequest>,
) -> Result<Json<ProviderMutationResponse>, ApiError> {
    let provider_name = payload.provider_name.trim();
    let base_url = payload.base_url.trim();
    let model_name = payload.model_name.trim();
    let api_key = payload
        .api_key
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    if provider_name.is_empty() || base_url.is_empty() || model_name.is_empty() {
        return Err(ApiError::invalid_argument(
            "provider fields must not be empty",
            json!({}),
        ));
    }
    let updated = state
        .update_provider(&provider_id, provider_name, base_url, model_name, api_key)?
        .ok_or_else(|| {
            ApiError::not_found("provider not found", json!({ "provider_id": provider_id }))
        })?;
    Ok(Json(ProviderMutationResponse {
        request_id: ctx.request_id,
        provider: to_provider_item(updated),
        timestamp: now_rfc3339(),
    }))
}

pub async fn delete_provider(
    State(state): State<AppState>,
    Path(provider_id): Path<String>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<ProviderDeleteResponse>, ApiError> {
    let deleted = state.delete_provider(&provider_id)?;
    if !deleted {
        return Err(ApiError::not_found(
            "provider not found",
            json!({ "provider_id": provider_id }),
        ));
    }
    Ok(Json(ProviderDeleteResponse {
        request_id: ctx.request_id,
        provider_id,
        status: "deleted",
        timestamp: now_rfc3339(),
    }))
}

fn to_sse_event(event: StreamEventEnvelope) -> SseEvent {
    SseEvent::default()
        .event(event.event_type.clone())
        .id(event.event_seq.to_string())
        .data(serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string()))
}

fn publish_event_for_session(state: &AppState, session: &mut crate::state::SessionRuntime, event: StreamEventEnvelope) {
    if let Err(err) = state.append_event(&event) {
        warn!(
            session_id = %event.session_id,
            event_seq = event.event_seq,
            event_type = %event.event_type,
            error = %err.message,
            "failed to persist event"
        );
    }
    session.publish_event(event);
}

fn to_provider_item(provider: crate::persistence::ProviderRecord) -> ProviderItem {
    ProviderItem {
        provider_id: provider.provider_id,
        provider_name: provider.provider_name,
        base_url: provider.base_url,
        model_name: provider.model_name,
        api_key_set: !provider.api_key.trim().is_empty(),
        created_at: provider.created_at,
        updated_at: provider.updated_at,
    }
}

fn to_active_provider_item(active: crate::persistence::ActiveProviderRecord) -> ActiveProviderItem {
    ActiveProviderItem {
        provider_id: active.provider_id,
        model_name: active.model_name,
        updated_at: active.updated_at,
    }
}

fn normalize_session_action(action: &str) -> String {
    action
        .trim_start_matches('/')
        .trim_start_matches(':')
        .to_string()
}

fn parse_session_action(session_action: &str) -> Result<(&str, &str), ApiError> {
    session_action.split_once(':').ok_or_else(|| {
        ApiError::not_found(
            "invalid session action route",
            json!({ "session_action": session_action }),
        )
    })
}

async fn clear_runtime(
    state: &AppState,
    session_runtime: &tokio::sync::Mutex<crate::state::SessionRuntime>,
) {
    let config = state.runtime_config();
    let mut session = session_runtime.lock().await;
    session.clear_context_with_config(config);
}

#[cfg(test)]
mod tests {
    use super::resolve_resume_seq;

    #[test]
    fn resolve_resume_seq_prefers_after_event_seq() {
        assert_eq!(resolve_resume_seq(Some(9), Some("3")), Some(9));
    }

    #[test]
    fn resolve_resume_seq_uses_last_event_id_when_query_absent() {
        assert_eq!(resolve_resume_seq(None, Some("7")), Some(7));
    }
}
