//! JSONL stdio daemon mode — used by `openjaxd` (thin shim) and the Python SDK.
//!
//! Entry point: `run_stdio()`. Reads JSONL from stdin, writes JSONL to stdout.

use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use async_trait::async_trait;
use openjax_core::{
    Agent, ApprovalHandler, ApprovalRequest, approval_timeout_ms_from_env, init_logger,
    load_runtime_config,
};
use openjax_protocol::{Event, Op};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio::time::{Duration, timeout};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

const PROTOCOL_VERSION: &str = "v1";
const KIND_REQUEST: &str = "request";
const KIND_RESPONSE: &str = "response";
const KIND_EVENT: &str = "event";
const USER_INPUT_LOG_PREVIEW_CHARS: usize = 200;

fn approval_bool_field(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "true",
        Some(false) => "false",
        None => "-",
    }
}

fn approval_text_field(value: Option<&str>) -> String {
    let raw = value.unwrap_or("-").trim();
    if raw.is_empty() {
        return "-".to_string();
    }
    raw.split_whitespace().collect::<Vec<_>>().join("_")
}

struct ApprovalLogEvent<'a> {
    action: &'a str,
    request_id: Option<&'a str>,
    turn_id: Option<&'a str>,
    target: Option<&'a str>,
    approved: Option<bool>,
    resolved: Option<bool>,
    session_id: Option<&'a str>,
    detail: Option<&'a str>,
}

fn log_approval_event(event: ApprovalLogEvent<'_>) {
    info!(
        "approval_event action={} request_id={} turn_id={} target={} approved={} resolved={} session_id={} detail={}",
        approval_text_field(Some(event.action)),
        approval_text_field(event.request_id),
        approval_text_field(event.turn_id),
        approval_text_field(event.target),
        approval_bool_field(event.approved),
        approval_bool_field(event.resolved),
        approval_text_field(event.session_id),
        approval_text_field(event.detail),
    );
}

#[derive(Debug, Deserialize)]
struct RequestEnvelope {
    protocol_version: String,
    kind: String,
    request_id: String,
    #[serde(default)]
    session_id: Option<String>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct ResponseEnvelope {
    protocol_version: &'static str,
    kind: &'static str,
    request_id: String,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorBody>,
}

#[derive(Debug, Serialize)]
struct EventEnvelope {
    protocol_version: &'static str,
    kind: &'static str,
    session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    turn_id: Option<String>,
    event_type: String,
    payload: Value,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    code: String,
    message: String,
    retriable: bool,
    details: Value,
}

struct SessionState {
    agent: Arc<Mutex<Agent>>,
    streaming_enabled: Arc<AtomicBool>,
    approval_handler: Arc<DaemonApprovalHandler>,
}

#[derive(Default)]
struct DaemonApprovalHandler {
    pending: Mutex<HashMap<String, oneshot::Sender<bool>>>,
}

impl DaemonApprovalHandler {
    async fn resolve(&self, request_id: &str, approved: bool) -> bool {
        let tx = {
            let mut pending = self.pending.lock().await;
            pending.remove(request_id)
        };
        match tx {
            Some(tx) => tx.send(approved).is_ok(),
            None => {
                warn!(approval_request_id = %request_id, "approval request not found");
                log_approval_event(ApprovalLogEvent {
                    action: "resolve_missing",
                    request_id: Some(request_id),
                    turn_id: None,
                    target: None,
                    approved: Some(approved),
                    resolved: Some(false),
                    session_id: None,
                    detail: Some("request_not_found"),
                });
                false
            }
        }
    }
}

#[async_trait]
impl ApprovalHandler for DaemonApprovalHandler {
    async fn request_approval(
        &self,
        request: ApprovalRequest,
    ) -> std::result::Result<bool, String> {
        let request_id = request.request_id.clone();
        let timeout_ms = approval_timeout_ms_from_env();
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(request_id.clone(), tx);
        }
        info!(approval_request_id = %request_id, "approval requested");
        log_approval_event(ApprovalLogEvent {
            action: "requested",
            request_id: Some(&request_id),
            turn_id: None,
            target: Some(&request.target),
            approved: None,
            resolved: None,
            session_id: None,
            detail: Some("handler_waiting"),
        });

        let decision = timeout(Duration::from_millis(timeout_ms), rx).await;
        let mut pending = self.pending.lock().await;
        pending.remove(&request_id);

        match decision {
            Ok(Ok(approved)) => {
                log_approval_event(ApprovalLogEvent {
                    action: "handler_decided",
                    request_id: Some(&request_id),
                    turn_id: None,
                    target: Some(&request.target),
                    approved: Some(approved),
                    resolved: Some(true),
                    session_id: None,
                    detail: Some("decision_received"),
                });
                Ok(approved)
            }
            Ok(Err(_)) => {
                warn!(approval_request_id = %request_id, "approval channel closed");
                log_approval_event(ApprovalLogEvent {
                    action: "handler_error",
                    request_id: Some(&request_id),
                    turn_id: None,
                    target: Some(&request.target),
                    approved: None,
                    resolved: Some(false),
                    session_id: None,
                    detail: Some("channel_closed"),
                });
                Err("approval channel closed".to_string())
            }
            Err(_) => {
                warn!(approval_request_id = %request_id, timeout_ms = timeout_ms, "approval timed out");
                log_approval_event(ApprovalLogEvent {
                    action: "handler_timeout",
                    request_id: Some(&request_id),
                    turn_id: None,
                    target: Some(&request.target),
                    approved: None,
                    resolved: Some(false),
                    session_id: None,
                    detail: Some("timeout"),
                });
                Err("approval timed out".to_string())
            }
        }
    }
}

/// Run the JSONL stdio daemon loop. Reads from stdin, writes to stdout.
/// Loads provider config from the database (same source as the WebUI).
pub async fn run_stdio() -> anyhow::Result<()> {
    init_logger();
    info!(
        component = "openjaxd",
        protocol_version = PROTOCOL_VERSION,
        "daemon started"
    );

    let stdin = BufReader::new(io::stdin());
    let mut lines = stdin.lines();
    let writer = Arc::new(Mutex::new(io::stdout()));

    let sessions: Arc<Mutex<HashMap<String, SessionState>>> = Arc::new(Mutex::new(HashMap::new()));

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        handle_line(&line, writer.clone(), sessions.clone()).await;
    }

    info!("stdin closed, cleaning up sessions");
    cleanup_sessions(sessions).await;
    info!("daemon exiting");

    Ok(())
}

async fn handle_line(
    line: &str,
    writer: Arc<Mutex<io::Stdout>>,
    sessions: Arc<Mutex<HashMap<String, SessionState>>>,
) {
    debug!(raw_line_len = line.len(), "received line");
    let raw: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(err) => {
            warn!(error = %err, "invalid json line");
            let _ = send_error(
                writer,
                "unknown".to_string(),
                "INVALID_REQUEST",
                format!("invalid JSON: {err}"),
                false,
                json!({}),
            )
            .await;
            return;
        }
    };

    let request_id = raw
        .get("request_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();

    let req: RequestEnvelope = match serde_json::from_value(raw) {
        Ok(v) => v,
        Err(err) => {
            warn!(request_id = %request_id, error = %err, "invalid request envelope");
            let _ = send_error(
                writer,
                request_id,
                "INVALID_REQUEST",
                format!("invalid request envelope: {err}"),
                false,
                json!({}),
            )
            .await;
            return;
        }
    };

    if req.protocol_version != PROTOCOL_VERSION || req.kind != KIND_REQUEST {
        warn!(
            request_id = %req.request_id,
            protocol_version = %req.protocol_version,
            kind = %req.kind,
            "unsupported request envelope"
        );
        let _ = send_error(
            writer,
            req.request_id,
            "INVALID_REQUEST",
            "unsupported protocol_version or kind".to_string(),
            false,
            json!({
                "protocol_version": req.protocol_version,
                "kind": req.kind
            }),
        )
        .await;
        return;
    }

    match req.method.as_str() {
        "start_session" => {
            info!(request_id = %req.request_id, method = "start_session", "handling request");
            let session_id = format!("sess_{}", Uuid::new_v4().simple());
            // Load provider config from DB (same source as WebUI).
            let config = load_runtime_config();
            let mut agent = Agent::with_config(config);
            let approval_handler = Arc::new(DaemonApprovalHandler::default());
            agent.set_approval_handler(approval_handler.clone());

            let state = SessionState {
                agent: Arc::new(Mutex::new(agent)),
                streaming_enabled: Arc::new(AtomicBool::new(false)),
                approval_handler,
            };

            sessions.lock().await.insert(session_id.clone(), state);
            info!(request_id = %req.request_id, session_id = %session_id, "session started");
            let result = json!({
                "session_id": session_id,
                "created_at": unix_now_str(),
            });
            let _ = send_ok(writer, req.request_id, result).await;
        }
        "stream_events" => {
            info!(request_id = %req.request_id, method = "stream_events", "handling request");
            let Some(session_id) = req.session_id else {
                let _ = send_error(
                    writer,
                    req.request_id,
                    "INVALID_PARAMS",
                    "session_id is required".to_string(),
                    false,
                    json!({}),
                )
                .await;
                return;
            };

            let mut sessions_guard = sessions.lock().await;
            let Some(state) = sessions_guard.get_mut(&session_id) else {
                let _ = send_error(
                    writer,
                    req.request_id,
                    "SESSION_NOT_FOUND",
                    "session not found".to_string(),
                    false,
                    json!({ "session_id": session_id }),
                )
                .await;
                return;
            };
            state.streaming_enabled.store(true, Ordering::Relaxed);
            info!(request_id = %req.request_id, session_id = %session_id, "stream enabled");
            let _ = send_ok(writer, req.request_id, json!({ "subscribed": true })).await;
        }
        "submit_turn" => {
            info!(request_id = %req.request_id, method = "submit_turn", "handling request");
            let Some(session_id) = req.session_id else {
                let _ = send_error(
                    writer,
                    req.request_id,
                    "INVALID_PARAMS",
                    "session_id is required".to_string(),
                    false,
                    json!({}),
                )
                .await;
                return;
            };

            let Some(input) = req.params.get("input").and_then(Value::as_str) else {
                let _ = send_error(
                    writer,
                    req.request_id,
                    "INVALID_PARAMS",
                    "params.input is required".to_string(),
                    false,
                    json!({}),
                )
                .await;
                return;
            };

            let (input_preview, input_truncated) =
                summarize_user_input(input, USER_INPUT_LOG_PREVIEW_CHARS);
            info!(
                request_id = %req.request_id,
                session_id = %session_id,
                input_len = input.chars().count(),
                input_preview = ?input_preview,
                input_truncated = input_truncated,
                "submit_turn accepted"
            );

            let sessions_guard = sessions.lock().await;
            let Some(state) = sessions_guard.get(&session_id) else {
                let _ = send_error(
                    writer,
                    req.request_id,
                    "SESSION_NOT_FOUND",
                    "session not found".to_string(),
                    false,
                    json!({ "session_id": session_id }),
                )
                .await;
                return;
            };

            let agent = state.agent.clone();
            let streaming_enabled = state.streaming_enabled.clone();
            let writer_for_events = writer.clone();
            let session_id_for_events = session_id.clone();
            let input_owned = input.to_string();
            let request_id = req.request_id.clone();

            tokio::spawn(async move {
                info!(request_id = %request_id, session_id = %session_id_for_events, "submit_turn worker started");
                let (event_tx, mut event_rx) = mpsc::unbounded_channel();
                let submit_agent = agent.clone();
                let submit_input = input_owned.clone();
                let submit_task = tokio::spawn(async move {
                    let mut agent = submit_agent.lock().await;
                    agent
                        .submit_with_sink(
                            Op::UserTurn {
                                input: submit_input,
                            },
                            event_tx,
                        )
                        .await
                });

                let mut response_sent = false;
                let mut response_turn_id: Option<String> = None;

                while let Some(event) = event_rx.recv().await {
                    if !response_sent && let Some(tid) = turn_id_from_event(&event) {
                        let tid_str = tid.to_string();
                        let response = send_ok(
                            writer_for_events.clone(),
                            request_id.clone(),
                            json!({"turn_id": tid_str, "accepted": true}),
                        )
                        .await;
                        if response.is_err() {
                            return;
                        }
                        response_sent = true;
                        response_turn_id = Some(tid.to_string());
                    }

                    if streaming_enabled.load(Ordering::Relaxed)
                        && let Some(envelope) = map_live_event(&session_id_for_events, event)
                    {
                        let _ = send_event(writer_for_events.clone(), envelope).await;
                    }
                }

                let events = match submit_task.await {
                    Ok(events) => events,
                    Err(err) => {
                        error!(request_id = %request_id, session_id = %session_id_for_events, error = %err, "submit task join failed");
                        let _ = send_error(
                            writer_for_events.clone(),
                            request_id,
                            "INTERNAL_ERROR",
                            "submit task failed".to_string(),
                            false,
                            json!({}),
                        )
                        .await;
                        return;
                    }
                };

                if !response_sent {
                    let response = if let Some(tid) =
                        first_turn_id(&events).map(|tid| tid.to_string())
                    {
                        response_turn_id = Some(tid.clone());
                        send_ok(
                            writer_for_events.clone(),
                            request_id.clone(),
                            json!({"turn_id": tid, "accepted": true}),
                        )
                        .await
                    } else {
                        error!(request_id = %request_id, session_id = %session_id_for_events, "failed to infer turn_id");
                        send_error(
                            writer_for_events.clone(),
                            request_id.clone(),
                            "INTERNAL_ERROR",
                            "failed to infer turn_id from events".to_string(),
                            false,
                            json!({}),
                        )
                        .await
                    };
                    if response.is_err() {
                        return;
                    }
                }

                if let Some(tid) = response_turn_id.as_deref() {
                    let (response_text_deltas, assistant_messages, tool_calls, approvals) =
                        summarize_turn_events(&events);
                    info!(
                        request_id = %request_id,
                        session_id = %session_id_for_events,
                        turn_id = %tid,
                        phase = "thinking_completed",
                        response_text_delta_events = response_text_deltas,
                        assistant_message_events = assistant_messages,
                        tool_call_events = tool_calls,
                        approval_events = approvals,
                        total_events = events.len(),
                        "turn lifecycle update"
                    );
                    info!(request_id = %request_id, session_id = %session_id_for_events, turn_id = %tid, "turn finished");
                }
            });
        }
        "resolve_approval" => {
            info!(request_id = %req.request_id, method = "resolve_approval", "handling request");
            let Some(session_id) = req.session_id else {
                let _ = send_error(
                    writer,
                    req.request_id,
                    "INVALID_PARAMS",
                    "session_id is required".to_string(),
                    false,
                    json!({}),
                )
                .await;
                return;
            };

            let Some(request_id_to_resolve) = req.params.get("request_id").and_then(Value::as_str)
            else {
                let _ = send_error(
                    writer,
                    req.request_id,
                    "INVALID_PARAMS",
                    "params.request_id is required".to_string(),
                    false,
                    json!({}),
                )
                .await;
                return;
            };

            let Some(approved) = req.params.get("approved").and_then(Value::as_bool) else {
                let _ = send_error(
                    writer,
                    req.request_id,
                    "INVALID_PARAMS",
                    "params.approved is required".to_string(),
                    false,
                    json!({}),
                )
                .await;
                return;
            };
            let turn_id_param = req.params.get("turn_id").and_then(Value::as_str);
            let target_param = req.params.get("target").and_then(Value::as_str);

            let approval_handler = {
                let sessions_guard = sessions.lock().await;
                let Some(state) = sessions_guard.get(&session_id) else {
                    let _ = send_error(
                        writer,
                        req.request_id,
                        "SESSION_NOT_FOUND",
                        "session not found".to_string(),
                        false,
                        json!({ "session_id": session_id }),
                    )
                    .await;
                    return;
                };
                state.approval_handler.clone()
            };

            let resolved = approval_handler
                .resolve(request_id_to_resolve, approved)
                .await;
            info!(
                request_id = %req.request_id,
                session_id = %session_id,
                approval_request_id = %request_id_to_resolve,
                approved = approved,
                resolved = resolved,
                "approval request processed"
            );
            log_approval_event(ApprovalLogEvent {
                action: "resolved_submit",
                request_id: Some(request_id_to_resolve),
                turn_id: turn_id_param,
                target: target_param,
                approved: Some(approved),
                resolved: Some(resolved),
                session_id: Some(&session_id),
                detail: Some("rpc_resolve_approval"),
            });

            if resolved {
                let _ = send_ok(writer, req.request_id, json!({ "resolved": true })).await;
            } else {
                let _ = send_error(
                    writer,
                    req.request_id,
                    "APPROVAL_NOT_FOUND",
                    "approval request not found or already resolved".to_string(),
                    false,
                    json!({ "request_id": request_id_to_resolve }),
                )
                .await;
            }
        }
        "shutdown_session" => {
            info!(request_id = %req.request_id, method = "shutdown_session", "handling request");
            let Some(session_id) = req.session_id else {
                let _ = send_error(
                    writer,
                    req.request_id,
                    "INVALID_PARAMS",
                    "session_id is required".to_string(),
                    false,
                    json!({}),
                )
                .await;
                return;
            };

            let state = sessions.lock().await.remove(&session_id);
            let Some(state) = state else {
                let _ = send_error(
                    writer,
                    req.request_id,
                    "SESSION_NOT_FOUND",
                    "session not found".to_string(),
                    false,
                    json!({ "session_id": session_id }),
                )
                .await;
                return;
            };

            let events = {
                let mut agent = state.agent.lock().await;
                agent.submit(Op::Shutdown).await
            };
            info!(request_id = %req.request_id, session_id = %session_id, "session shutdown complete");

            let _ = send_ok(writer.clone(), req.request_id, json!({ "closed": true })).await;

            if state.streaming_enabled.load(Ordering::Relaxed) {
                for event in events {
                    if let Some(envelope) = map_event(&session_id, event) {
                        let _ = send_event(writer.clone(), envelope).await;
                    }
                }
            }
        }
        _ => {
            warn!(request_id = %req.request_id, method = %req.method, "unsupported method");
            let _ = send_error(
                writer,
                req.request_id,
                "NOT_IMPLEMENTED",
                "unsupported method".to_string(),
                false,
                json!({ "method": req.method }),
            )
            .await;
        }
    }
}

fn first_turn_id(events: &[Event]) -> Option<u64> {
    for event in events {
        match event {
            Event::TurnStarted { turn_id }
            | Event::ToolCallStarted { turn_id, .. }
            | Event::ToolCallCompleted { turn_id, .. }
            | Event::ToolCallArgsDelta { turn_id, .. }
            | Event::ToolCallReady { turn_id, .. }
            | Event::ToolCallProgress { turn_id, .. }
            | Event::ToolCallFailed { turn_id, .. }
            | Event::AssistantMessage { turn_id, .. }
            | Event::ResponseStarted { turn_id, .. }
            | Event::ResponseTextDelta { turn_id, .. }
            | Event::ReasoningDelta { turn_id, .. }
            | Event::ResponseCompleted { turn_id, .. }
            | Event::ResponseError { turn_id, .. }
            | Event::ResponseResumed { turn_id, .. }
            | Event::ToolCallsProposed { turn_id, .. }
            | Event::ToolBatchCompleted { turn_id, .. }
            | Event::ApprovalRequested { turn_id, .. }
            | Event::ApprovalResolved { turn_id, .. }
            | Event::TurnCompleted { turn_id } => return Some(*turn_id),
            Event::AgentSpawned { .. }
            | Event::AgentStatusChanged { .. }
            | Event::ContextUsageUpdated { .. }
            | Event::ContextCompacted { .. }
            | Event::LoopWarning { .. }
            | Event::ShutdownComplete => {}
        }
    }
    None
}

fn turn_id_from_event(event: &Event) -> Option<u64> {
    match event {
        Event::TurnStarted { turn_id }
        | Event::ToolCallStarted { turn_id, .. }
        | Event::ToolCallCompleted { turn_id, .. }
        | Event::ToolCallArgsDelta { turn_id, .. }
        | Event::ToolCallReady { turn_id, .. }
        | Event::ToolCallProgress { turn_id, .. }
        | Event::ToolCallFailed { turn_id, .. }
        | Event::AssistantMessage { turn_id, .. }
        | Event::ResponseStarted { turn_id, .. }
        | Event::ResponseTextDelta { turn_id, .. }
        | Event::ReasoningDelta { turn_id, .. }
        | Event::ResponseCompleted { turn_id, .. }
        | Event::ResponseError { turn_id, .. }
        | Event::ResponseResumed { turn_id, .. }
        | Event::ToolCallsProposed { turn_id, .. }
        | Event::ToolBatchCompleted { turn_id, .. }
        | Event::ApprovalRequested { turn_id, .. }
        | Event::ApprovalResolved { turn_id, .. }
        | Event::TurnCompleted { turn_id } => Some(*turn_id),
        Event::AgentSpawned { .. }
        | Event::AgentStatusChanged { .. }
        | Event::ContextUsageUpdated { .. }
        | Event::ContextCompacted { .. }
        | Event::LoopWarning { .. }
        | Event::ShutdownComplete => None,
    }
}

fn map_event(session_id: &str, event: Event) -> Option<EventEnvelope> {
    match event {
        Event::TurnStarted { turn_id } => Some(EventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            kind: KIND_EVENT,
            session_id: session_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            event_type: "turn_started".to_string(),
            payload: json!({}),
        }),
        Event::ToolCallStarted {
            turn_id,
            tool_name,
            target,
            ..
        } => Some(EventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            kind: KIND_EVENT,
            session_id: session_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            event_type: "tool_call_started".to_string(),
            payload: json!({ "tool_name": tool_name, "target": target }),
        }),
        Event::ToolCallCompleted {
            turn_id,
            tool_name,
            ok,
            output,
            ..
        } => Some(EventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            kind: KIND_EVENT,
            session_id: session_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            event_type: "tool_call_completed".to_string(),
            payload: json!({ "tool_name": tool_name, "ok": ok, "output": output }),
        }),
        Event::ToolCallArgsDelta {
            turn_id,
            tool_name,
            args_delta,
            ..
        } => Some(EventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            kind: KIND_EVENT,
            session_id: session_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            event_type: "tool_args_delta".to_string(),
            payload: json!({ "tool_name": tool_name, "args_delta": args_delta }),
        }),
        Event::ToolCallProgress {
            turn_id,
            tool_name,
            progress_message,
            ..
        } => Some(EventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            kind: KIND_EVENT,
            session_id: session_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            event_type: "tool_call_progress".to_string(),
            payload: json!({ "tool_name": tool_name, "progress_message": progress_message }),
        }),
        Event::ToolCallReady {
            turn_id, tool_name, ..
        } => Some(EventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            kind: KIND_EVENT,
            session_id: session_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            event_type: "tool_call_ready".to_string(),
            payload: json!({ "tool_name": tool_name }),
        }),
        Event::ToolCallFailed {
            turn_id,
            tool_name,
            code,
            message,
            retryable,
            ..
        } => Some(EventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            kind: KIND_EVENT,
            session_id: session_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            event_type: "tool_call_failed".to_string(),
            payload: json!({ "tool_name": tool_name, "code": code, "message": message, "retryable": retryable }),
        }),
        Event::AssistantMessage { turn_id, content } => Some(EventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            kind: KIND_EVENT,
            session_id: session_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            event_type: "assistant_message".to_string(),
            payload: json!({ "content": content }),
        }),
        Event::ResponseStarted {
            turn_id,
            stream_source,
        } => Some(EventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            kind: KIND_EVENT,
            session_id: session_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            event_type: "response_started".to_string(),
            payload: json!({ "stream_source": stream_source }),
        }),
        Event::ResponseTextDelta {
            turn_id,
            content_delta,
            stream_source,
        } => Some(EventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            kind: KIND_EVENT,
            session_id: session_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            event_type: "response_text_delta".to_string(),
            payload: json!({ "content_delta": content_delta, "stream_source": stream_source }),
        }),
        Event::ReasoningDelta {
            turn_id,
            content_delta,
            stream_source,
        } => Some(EventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            kind: KIND_EVENT,
            session_id: session_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            event_type: "reasoning_delta".to_string(),
            payload: json!({ "content_delta": content_delta, "stream_source": stream_source }),
        }),
        Event::ResponseCompleted {
            turn_id,
            content,
            stream_source,
        } => Some(EventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            kind: KIND_EVENT,
            session_id: session_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            event_type: "response_completed".to_string(),
            payload: json!({ "content": content, "stream_source": stream_source }),
        }),
        Event::ResponseError {
            turn_id,
            code,
            message,
            retryable,
        } => Some(EventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            kind: KIND_EVENT,
            session_id: session_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            event_type: "response_error".to_string(),
            payload: json!({ "code": code, "message": message, "retryable": retryable }),
        }),
        Event::ResponseResumed {
            turn_id,
            stream_source,
        } => Some(EventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            kind: KIND_EVENT,
            session_id: session_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            event_type: "response_resumed".to_string(),
            payload: json!({ "stream_source": stream_source }),
        }),
        Event::ToolCallsProposed {
            turn_id,
            tool_calls,
        } => Some(EventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            kind: KIND_EVENT,
            session_id: session_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            event_type: "tool_calls_proposed".to_string(),
            payload: json!({ "tool_calls": tool_calls }),
        }),
        Event::ToolBatchCompleted {
            turn_id,
            total,
            succeeded,
            failed,
        } => Some(EventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            kind: KIND_EVENT,
            session_id: session_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            event_type: "tool_batch_completed".to_string(),
            payload: json!({ "total": total, "succeeded": succeeded, "failed": failed }),
        }),
        Event::ApprovalRequested {
            turn_id,
            request_id,
            target,
            reason,
            tool_name,
            command_preview,
            risk_tags,
            sandbox_backend,
            degrade_reason,
            policy_version,
            matched_rule_id,
        } => Some(EventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            kind: KIND_EVENT,
            session_id: session_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            event_type: "approval_requested".to_string(),
            payload: json!({
                "request_id": request_id,
                "target": target,
                "reason": reason,
                "tool_name": tool_name,
                "command_preview": command_preview,
                "risk_tags": risk_tags,
                "sandbox_backend": sandbox_backend,
                "degrade_reason": degrade_reason,
                "policy_version": policy_version,
                "matched_rule_id": matched_rule_id
            }),
        }),
        Event::ApprovalResolved {
            turn_id,
            request_id,
            approved,
        } => Some(EventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            kind: KIND_EVENT,
            session_id: session_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            event_type: "approval_resolved".to_string(),
            payload: json!({ "request_id": request_id, "approved": approved }),
        }),
        Event::TurnCompleted { turn_id } => Some(EventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            kind: KIND_EVENT,
            session_id: session_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            event_type: "turn_completed".to_string(),
            payload: json!({}),
        }),
        Event::ShutdownComplete => Some(EventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            kind: KIND_EVENT,
            session_id: session_id.to_string(),
            turn_id: None,
            event_type: "session_shutdown_complete".to_string(),
            payload: json!({}),
        }),
        Event::AgentSpawned { .. } | Event::AgentStatusChanged { .. } => None,
        Event::ContextUsageUpdated { .. } => None,
        // TODO: emit proper SSE envelope for ContextCompacted (gateway HTTP path handled in event_mapper)
        Event::ContextCompacted { .. } => None,
        Event::LoopWarning { .. } => None,
    }
}

fn map_live_event(session_id: &str, event: Event) -> Option<EventEnvelope> {
    let envelope = map_event(session_id, event)?;
    if envelope.event_type == "assistant_message" {
        return None;
    }
    Some(envelope)
}

async fn send_ok(
    writer: Arc<Mutex<io::Stdout>>,
    request_id: String,
    result: Value,
) -> anyhow::Result<()> {
    let envelope = ResponseEnvelope {
        protocol_version: PROTOCOL_VERSION,
        kind: KIND_RESPONSE,
        request_id,
        ok: true,
        result: Some(result),
        error: None,
    };
    write_line(writer, &envelope).await
}

async fn send_error(
    writer: Arc<Mutex<io::Stdout>>,
    request_id: String,
    code: &str,
    message: String,
    retriable: bool,
    details: Value,
) -> anyhow::Result<()> {
    let envelope = ResponseEnvelope {
        protocol_version: PROTOCOL_VERSION,
        kind: KIND_RESPONSE,
        request_id,
        ok: false,
        result: None,
        error: Some(ErrorBody {
            code: code.to_string(),
            message,
            retriable,
            details,
        }),
    };
    write_line(writer, &envelope).await
}

async fn send_event(writer: Arc<Mutex<io::Stdout>>, event: EventEnvelope) -> anyhow::Result<()> {
    write_line(writer, &event).await
}

async fn write_line<T: Serialize>(writer: Arc<Mutex<io::Stdout>>, value: &T) -> anyhow::Result<()> {
    let mut out = writer.lock().await;
    let mut line = serde_json::to_vec(value)?;
    line.push(b'\n');
    out.write_all(&line).await?;
    out.flush().await?;
    Ok(())
}

fn unix_now_str() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs().to_string(),
        Err(_) => "0".to_string(),
    }
}

fn summarize_user_input(input: &str, preview_limit: usize) -> (String, bool) {
    let normalized = input.replace('\n', "\\n").replace('\r', "\\r");
    let total = normalized.chars().count();
    if total <= preview_limit {
        return (normalized, false);
    }
    let mut preview = normalized.chars().take(preview_limit).collect::<String>();
    preview.push_str("...");
    (preview, true)
}

fn summarize_turn_events(events: &[Event]) -> (usize, usize, usize, usize) {
    let mut response_text_deltas = 0usize;
    let mut assistant_messages = 0usize;
    let mut tool_calls = 0usize;
    let mut approvals = 0usize;

    for event in events {
        match event {
            Event::ResponseTextDelta { .. } => response_text_deltas += 1,
            Event::ReasoningDelta { .. } => response_text_deltas += 1,
            Event::AssistantMessage { .. } => assistant_messages += 1,
            Event::ToolCallStarted { .. }
            | Event::ToolCallCompleted { .. }
            | Event::ToolCallArgsDelta { .. }
            | Event::ToolCallReady { .. }
            | Event::ToolCallProgress { .. }
            | Event::ToolCallFailed { .. } => tool_calls += 1,
            Event::ApprovalRequested { .. } | Event::ApprovalResolved { .. } => approvals += 1,
            Event::TurnStarted { .. }
            | Event::TurnCompleted { .. }
            | Event::ResponseStarted { .. }
            | Event::ResponseCompleted { .. }
            | Event::ResponseError { .. }
            | Event::ResponseResumed { .. }
            | Event::ToolCallsProposed { .. }
            | Event::ToolBatchCompleted { .. }
            | Event::AgentSpawned { .. }
            | Event::AgentStatusChanged { .. }
            | Event::ContextUsageUpdated { .. }
            | Event::ContextCompacted { .. }
            | Event::LoopWarning { .. }
            | Event::ShutdownComplete => {}
        }
    }

    (
        response_text_deltas,
        assistant_messages,
        tool_calls,
        approvals,
    )
}

async fn cleanup_sessions(sessions: Arc<Mutex<HashMap<String, SessionState>>>) {
    let all_sessions = {
        let mut guard = sessions.lock().await;
        std::mem::take(&mut *guard)
    };
    for (session_id, state) in all_sessions {
        info!(session_id = %session_id, "cleaning session");
        let mut agent = state.agent.lock().await;
        let _ = agent.submit(Op::Shutdown).await;
    }
}

#[cfg(test)]
mod tests {
    use super::{map_event, map_live_event, summarize_turn_events, summarize_user_input};
    use openjax_protocol::Event;

    #[test]
    fn summarize_user_input_marks_truncated_preview() {
        let (preview, truncated) = summarize_user_input("abcdef", 3);
        assert_eq!(preview, "abc...");
        assert!(truncated);
    }

    #[test]
    fn summarize_user_input_escapes_newlines() {
        let (preview, truncated) = summarize_user_input("hello\nworld", 40);
        assert_eq!(preview, "hello\\nworld");
        assert!(!truncated);
    }

    #[test]
    fn summarize_turn_events_counts_key_event_types() {
        let events = vec![
            Event::TurnStarted { turn_id: 1 },
            Event::ResponseTextDelta {
                turn_id: 1,
                content_delta: "A".to_string(),
                stream_source: openjax_protocol::StreamSource::ModelLive,
            },
            Event::AssistantMessage {
                turn_id: 1,
                content: "B".to_string(),
            },
            Event::ToolCallStarted {
                turn_id: 1,
                tool_call_id: "tc_1".to_string(),
                tool_name: "read_file".to_string(),
                target: None,
                display_name: None,
            },
            Event::ToolCallCompleted {
                turn_id: 1,
                tool_call_id: "tc_1".to_string(),
                tool_name: "read_file".to_string(),
                ok: true,
                output: "ok".to_string(),
                display_name: None,
            },
            Event::ApprovalRequested {
                turn_id: 1,
                request_id: "r1".to_string(),
                target: "command".to_string(),
                reason: "confirm".to_string(),
                tool_name: Some("shell".to_string()),
                command_preview: Some("echo hi".to_string()),
                risk_tags: vec!["write".to_string()],
                sandbox_backend: Some("linux_native".to_string()),
                degrade_reason: None,
                policy_version: Some(1),
                matched_rule_id: Some("rule_1".to_string()),
            },
            Event::ApprovalResolved {
                turn_id: 1,
                request_id: "r1".to_string(),
                approved: true,
            },
            Event::TurnCompleted { turn_id: 1 },
        ];

        let (response_deltas, messages, tools, approvals) = summarize_turn_events(&events);
        assert_eq!(response_deltas, 1);
        assert_eq!(messages, 1);
        assert_eq!(tools, 2);
        assert_eq!(approvals, 2);
    }

    #[test]
    fn normal_turn_stream_has_no_assistant_message_event() {
        let assistant_message = Event::AssistantMessage {
            turn_id: 7,
            content: "final".to_string(),
        };

        let live_event = map_live_event("sess_1", assistant_message.clone());
        assert!(live_event.is_none());

        let compat_event = map_event("sess_1", assistant_message)
            .expect("shared compatibility mapping still exists");
        assert_eq!(compat_event.event_type, "assistant_message");

        let response_completed = map_live_event(
            "sess_1",
            Event::ResponseCompleted {
                turn_id: 7,
                content: "final".to_string(),
                stream_source: openjax_protocol::StreamSource::Synthetic,
            },
        )
        .expect("response_completed still streams");
        assert_eq!(response_completed.event_type, "response_completed");
    }
}
