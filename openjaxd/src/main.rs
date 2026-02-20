use anyhow::Result;
use async_trait::async_trait;
use openjax_core::{Agent, ApprovalHandler, ApprovalRequest, Config, init_logger};
use openjax_protocol::{Event, Op};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{Mutex, oneshot};
use tokio::time::{Duration, timeout};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

const PROTOCOL_VERSION: &str = "v1";
const KIND_REQUEST: &str = "request";
const KIND_RESPONSE: &str = "response";
const KIND_EVENT: &str = "event";
const APPROVAL_TIMEOUT_MS: u64 = 60_000;

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
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(request_id.clone(), tx);
        }
        info!(approval_request_id = %request_id, "approval requested");

        let decision = timeout(Duration::from_millis(APPROVAL_TIMEOUT_MS), rx).await;
        let mut pending = self.pending.lock().await;
        pending.remove(&request_id);

        match decision {
            Ok(Ok(approved)) => Ok(approved),
            Ok(Err(_)) => {
                warn!(approval_request_id = %request_id, "approval channel closed");
                Err("approval channel closed".to_string())
            }
            Err(_) => {
                warn!(approval_request_id = %request_id, timeout_ms = APPROVAL_TIMEOUT_MS, "approval timed out");
                Err("approval timed out".to_string())
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
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
            let mut agent = Agent::with_config(Config::load());
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
                "created_at": chrono_like_now(),
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
                let events = {
                    let mut agent = agent.lock().await;
                    agent.submit(Op::UserTurn { input: input_owned }).await
                };

                let turn_id = first_turn_id(&events).map(|tid| tid.to_string());
                let response = if let Some(tid) = turn_id.clone() {
                    info!(request_id = %request_id, session_id = %session_id_for_events, turn_id = %tid, "turn finished");
                    send_ok(
                        writer_for_events.clone(),
                        request_id,
                        json!({"turn_id": tid, "accepted": true}),
                    )
                    .await
                } else {
                    error!(request_id = %request_id, session_id = %session_id_for_events, "failed to infer turn_id");
                    send_error(
                        writer_for_events.clone(),
                        request_id,
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

                if streaming_enabled.load(Ordering::Relaxed) {
                    for event in events {
                        if let Some(envelope) = map_event(&session_id_for_events, event) {
                            let _ = send_event(writer_for_events.clone(), envelope).await;
                        }
                    }
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
            | Event::AssistantMessage { turn_id, .. }
            | Event::AssistantDelta { turn_id, .. }
            | Event::ApprovalRequested { turn_id, .. }
            | Event::ApprovalResolved { turn_id, .. }
            | Event::TurnCompleted { turn_id } => return Some(*turn_id),
            Event::AgentSpawned { .. }
            | Event::AgentStatusChanged { .. }
            | Event::ShutdownComplete => {}
        }
    }
    None
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
        Event::ToolCallStarted { turn_id, tool_name } => Some(EventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            kind: KIND_EVENT,
            session_id: session_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            event_type: "tool_call_started".to_string(),
            payload: json!({ "tool_name": tool_name }),
        }),
        Event::ToolCallCompleted {
            turn_id,
            tool_name,
            ok,
            output,
        } => Some(EventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            kind: KIND_EVENT,
            session_id: session_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            event_type: "tool_call_completed".to_string(),
            payload: json!({ "tool_name": tool_name, "ok": ok, "output": output }),
        }),
        Event::AssistantMessage { turn_id, content } => Some(EventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            kind: KIND_EVENT,
            session_id: session_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            event_type: "assistant_message".to_string(),
            payload: json!({ "content": content }),
        }),
        Event::AssistantDelta {
            turn_id,
            content_delta,
        } => Some(EventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            kind: KIND_EVENT,
            session_id: session_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            event_type: "assistant_delta".to_string(),
            payload: json!({ "content_delta": content_delta }),
        }),
        Event::ApprovalRequested {
            turn_id,
            request_id,
            target,
            reason,
        } => Some(EventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            kind: KIND_EVENT,
            session_id: session_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            event_type: "approval_requested".to_string(),
            payload: json!({
                "request_id": request_id,
                "target": target,
                "reason": reason
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
    }
}

async fn send_ok(writer: Arc<Mutex<io::Stdout>>, request_id: String, result: Value) -> Result<()> {
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
) -> Result<()> {
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

async fn send_event(writer: Arc<Mutex<io::Stdout>>, event: EventEnvelope) -> Result<()> {
    write_line(writer, &event).await
}

async fn write_line<T: Serialize>(writer: Arc<Mutex<io::Stdout>>, value: &T) -> Result<()> {
    let mut out = writer.lock().await;
    let mut line = serde_json::to_vec(value)?;
    line.push(b'\n');
    out.write_all(&line).await?;
    out.flush().await?;
    Ok(())
}

fn chrono_like_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs().to_string(),
        Err(_) => "0".to_string(),
    }
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
