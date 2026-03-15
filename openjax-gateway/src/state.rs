use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Instant;

use async_trait::async_trait;
use openjax_core::streaming::ReplayBuffer;
use openjax_core::{Agent, ApprovalHandler, ApprovalRequest, Config, approval_timeout_ms_from_env};
use openjax_protocol::Event;
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::{Mutex, RwLock, broadcast, mpsc, oneshot};
use tokio::time::{Duration, timeout};
use tracing::{info, warn};
use uuid::Uuid;

use crate::auth::load_api_keys_from_env;
use crate::auth::{AuthConfig, AuthService};
use crate::error::{ApiError, now_rfc3339};
use crate::event_mapper::map_core_event_payload;

const DEFAULT_EVENT_REPLAY_LIMIT: usize = 1024;
const DEFAULT_EVENT_CHANNEL_CAPACITY: usize = 1024;
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

fn log_preview(text: &str, max_chars: usize) -> (String, bool) {
    let mut iter = text.chars();
    let preview: String = iter.by_ref().take(max_chars).collect();
    let truncated = iter.next().is_some();
    (preview, truncated)
}

#[derive(Clone)]
pub struct AppState {
    sessions: Arc<RwLock<HashMap<String, Arc<Mutex<SessionRuntime>>>>>,
    api_keys: Arc<HashSet<String>>,
    auth_service: Arc<AuthService>,
}

impl AppState {
    pub fn new() -> Self {
        Self::new_with_api_keys(load_api_keys_from_env())
    }

    pub fn new_with_api_keys(api_keys: HashSet<String>) -> Self {
        Self::try_new_with_api_keys(api_keys).expect("initialize gateway auth service")
    }

    pub fn try_new_with_api_keys(api_keys: HashSet<String>) -> anyhow::Result<Self> {
        let auth_config = AuthConfig::from_env();
        let auth_service = if auth_config.db_path.as_os_str() == ":memory:" {
            AuthService::for_test()?
        } else {
            AuthService::from_config(auth_config.clone())?
        };
        Ok(Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            api_keys: Arc::new(api_keys),
            auth_service: Arc::new(auth_service),
        })
    }

    pub fn new_with_api_keys_for_test(api_keys: HashSet<String>) -> Self {
        let auth_service = AuthService::for_test().expect("initialize test auth service");
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            api_keys: Arc::new(api_keys),
            auth_service: Arc::new(auth_service),
        }
    }

    pub fn is_api_key_allowed(&self, key: &str) -> bool {
        self.api_keys.contains(key)
    }

    pub fn auth_service(&self) -> Arc<AuthService> {
        self.auth_service.clone()
    }

    pub async fn create_session(&self) -> String {
        let session_id = format!("sess_{}", Uuid::new_v4().simple());
        let runtime = Arc::new(Mutex::new(SessionRuntime::new()));
        self.sessions
            .write()
            .await
            .insert(session_id.clone(), runtime);
        session_id
    }

    pub async fn get_session(
        &self,
        session_id: &str,
    ) -> Result<Arc<Mutex<SessionRuntime>>, ApiError> {
        self.sessions
            .read()
            .await
            .get(session_id)
            .cloned()
            .ok_or_else(|| {
                ApiError::not_found("session not found", json!({ "session_id": session_id }))
            })
    }

    pub async fn remove_session(&self, session_id: &str) -> Option<Arc<Mutex<SessionRuntime>>> {
        self.sessions.write().await.remove(session_id)
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SessionRuntime {
    pub agent: Arc<Mutex<Agent>>,
    pub approval_handler: Arc<GatewayApprovalHandler>,
    pub status: SessionStatus,
    pub turns: HashMap<String, TurnRuntime>,
    pub core_turn_to_public: HashMap<u64, String>,
    pub next_event_seq: u64,
    pub turn_event_seq: HashMap<String, u64>,
    pub event_log: ReplayBuffer<StreamEventEnvelope>,
    pub event_tx: broadcast::Sender<StreamEventEnvelope>,
    pub resolved_approvals: HashSet<String>,
    last_event_emitted_at: Option<Instant>,
    replay_capacity: usize,
}

impl SessionRuntime {
    pub fn new() -> Self {
        let mut agent = Agent::with_config(Config::load());
        let approval_handler = Arc::new(GatewayApprovalHandler::default());
        agent.set_approval_handler(approval_handler.clone());
        let replay_capacity = event_replay_limit();
        let channel_capacity = event_channel_capacity();
        let (event_tx, _) = broadcast::channel(channel_capacity);
        Self {
            agent: Arc::new(Mutex::new(agent)),
            approval_handler,
            status: SessionStatus::Active,
            turns: HashMap::new(),
            core_turn_to_public: HashMap::new(),
            next_event_seq: 1,
            turn_event_seq: HashMap::new(),
            event_log: ReplayBuffer::with_capacity(replay_capacity),
            event_tx,
            resolved_approvals: HashSet::new(),
            last_event_emitted_at: None,
            replay_capacity,
        }
    }

    pub fn clear_context(&mut self) {
        let mut agent = Agent::with_config(Config::load());
        let approval_handler = Arc::new(GatewayApprovalHandler::default());
        agent.set_approval_handler(approval_handler.clone());
        self.agent = Arc::new(Mutex::new(agent));
        self.approval_handler = approval_handler;
        self.status = SessionStatus::Active;
        self.turns.clear();
        self.core_turn_to_public.clear();
        self.resolved_approvals.clear();
        self.turn_event_seq.clear();
        self.event_log = ReplayBuffer::with_capacity(self.replay_capacity);
        self.last_event_emitted_at = None;
    }

    pub fn create_gateway_event(
        &mut self,
        request_id: &str,
        session_id: &str,
        turn_id: Option<String>,
        event_type: &str,
        payload: Value,
        stream_source: Option<&str>,
    ) -> StreamEventEnvelope {
        let turn_seq = if let Some(turn_id) = &turn_id {
            let seq = self.turn_event_seq.entry(turn_id.clone()).or_insert(0);
            *seq += 1;
            *seq
        } else {
            0
        };
        let event = StreamEventEnvelope {
            request_id: request_id.to_string(),
            session_id: session_id.to_string(),
            turn_id,
            event_seq: self.next_event_seq,
            turn_seq,
            timestamp: now_rfc3339(),
            event_type: event_type.to_string(),
            stream_source: stream_source.unwrap_or("synthetic").to_string(),
            payload,
        };
        self.next_event_seq += 1;
        event
    }

    pub fn publish_event(&mut self, event: StreamEventEnvelope) {
        let _ = self.event_log.push(event.clone());
        let _ = self.event_tx.send(event);
    }

    pub fn replay_from(
        &self,
        after_event_seq: Option<u64>,
    ) -> Result<Vec<StreamEventEnvelope>, ApiError> {
        let replay = self.event_log.replay_from(after_event_seq).map_err(|err| {
            ApiError::invalid_argument(
                "replay point is outside retention window",
                json!({ "after_event_seq": err.requested_after_seq, "min_allowed": err.min_allowed }),
            )
        })?;
        Ok(replay.into_iter().map(|(_, event)| event).collect())
    }

    pub fn get_or_create_public_turn_id(&mut self, core_turn_id: u64) -> String {
        if let Some(id) = self.core_turn_to_public.get(&core_turn_id) {
            return id.clone();
        }
        let public_id = format!("turn_{}", core_turn_id);
        self.core_turn_to_public
            .insert(core_turn_id, public_id.clone());
        public_id
    }
}

fn event_replay_limit() -> usize {
    std::env::var("OPENJAX_GATEWAY_EVENT_REPLAY_LIMIT")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_EVENT_REPLAY_LIMIT)
}

fn event_channel_capacity() -> usize {
    std::env::var("OPENJAX_GATEWAY_EVENT_CHANNEL_CAPACITY")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_EVENT_CHANNEL_CAPACITY)
}

impl Default for SessionRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Active,
    Closing,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct TurnRuntime {
    pub status: TurnStatus,
    pub assistant_message: Option<String>,
    pub error: Option<ApiTurnError>,
}

impl TurnRuntime {
    pub fn queued() -> Self {
        Self {
            status: TurnStatus::Queued,
            assistant_message: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiTurnError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub details: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct StreamEventEnvelope {
    pub request_id: String,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    pub event_seq: u64,
    #[serde(default)]
    pub turn_seq: u64,
    pub timestamp: String,
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(default)]
    pub stream_source: String,
    pub payload: Value,
}

#[derive(Default)]
pub struct GatewayApprovalHandler {
    pending: Mutex<HashMap<String, oneshot::Sender<bool>>>,
}

impl GatewayApprovalHandler {
    pub async fn resolve(&self, approval_id: &str, approved: bool) -> bool {
        let tx = self.pending.lock().await.remove(approval_id);
        match tx {
            Some(tx) => tx.send(approved).is_ok(),
            None => false,
        }
    }
}

#[async_trait]
impl ApprovalHandler for GatewayApprovalHandler {
    async fn request_approval(&self, request: ApprovalRequest) -> Result<bool, String> {
        let timeout_ms = approval_timeout_ms_from_env();
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .await
            .insert(request.request_id.clone(), tx);
        info!(
            approval_id = %request.request_id,
            target = %request.target,
            "approval requested"
        );
        match timeout(Duration::from_millis(timeout_ms), rx).await {
            Ok(Ok(approved)) => Ok(approved),
            Ok(Err(_)) => Err("approval channel closed".to_string()),
            Err(_) => {
                warn!(approval_id = %request.request_id, "approval timed out");
                Err("approval timed out".to_string())
            }
        }
    }
}

pub async fn run_turn_task(
    session_runtime: Arc<Mutex<SessionRuntime>>,
    session_id: String,
    request_id: String,
    input: String,
    turn_id_tx: oneshot::Sender<Result<String, ApiError>>,
) {
    let (event_sink_tx, mut event_sink_rx) = mpsc::unbounded_channel();
    let agent = {
        let guard = session_runtime.lock().await;
        guard.agent.clone()
    };

    let submit_task = tokio::spawn(async move {
        let mut guard = agent.lock().await;
        guard
            .submit_with_sink(openjax_protocol::Op::UserTurn { input }, event_sink_tx)
            .await
    });

    let mut sent_turn_id = false;
    let mut pending_turn_id_tx = Some(turn_id_tx);
    while let Some(event) = event_sink_rx.recv().await {
        let mapped = {
            let mut session = session_runtime.lock().await;
            map_core_event(
                &mut session,
                &session_id,
                &request_id,
                event,
                &mut pending_turn_id_tx,
            )
        };
        if mapped.is_some() {
            sent_turn_id = true;
        }
    }

    match submit_task.await {
        Ok(events) => {
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
        Err(_) => {
            if let Some(tx) = pending_turn_id_tx.take() {
                let _ = tx.send(Err(ApiError::upstream_unavailable(
                    "core execution task failed",
                    json!({}),
                )));
            }
        }
    }
}

fn map_core_event(
    session: &mut SessionRuntime,
    session_id: &str,
    request_id: &str,
    event: Event,
    turn_id_tx: &mut Option<oneshot::Sender<Result<String, ApiError>>>,
) -> Option<String> {
    let mapping = map_core_event_payload(&event)?;
    let core_turn_id = mapping.core_turn_id;
    let event_type = mapping.event_type;
    let payload = mapping.payload;
    let stream_source = mapping.stream_source;

    let public_turn_id = core_turn_id.map(|tid| session.get_or_create_public_turn_id(tid));
    if let Some(turn_id) = &public_turn_id {
        let turn = session
            .turns
            .entry(turn_id.clone())
            .or_insert_with(TurnRuntime::queued);
        if event_type == "turn_started" {
            turn.status = TurnStatus::Running;
        } else if event_type == "turn_completed" {
            if !matches!(turn.status, TurnStatus::Failed) {
                turn.status = TurnStatus::Completed;
            }
        } else if event_type == "response_error" {
            turn.status = TurnStatus::Failed;
            turn.error = Some(ApiTurnError {
                code: payload
                    .get("code")
                    .and_then(|value| value.as_str())
                    .unwrap_or("UPSTREAM_ERROR")
                    .to_string(),
                message: payload
                    .get("message")
                    .and_then(|value| value.as_str())
                    .unwrap_or("response failed")
                    .to_string(),
                retryable: payload
                    .get("retryable")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false),
                details: payload.clone(),
            });
        } else if event_type == "response_completed"
            && let Some(content) = payload.get("content").and_then(|value| value.as_str())
        {
            turn.assistant_message = Some(content.to_string());
        }
    }

    if let Some(turn_id) = public_turn_id.clone()
        && let Some(tx) = turn_id_tx.take()
    {
        let _ = tx.send(Ok(turn_id));
    }

    let envelope = session.create_gateway_event(
        request_id,
        session_id,
        public_turn_id.clone(),
        event_type,
        payload,
        stream_source,
    );
    if gateway_stream_debug_enabled()
        && matches!(
            event_type,
            "response_started"
                | "response_text_delta"
                | "response_completed"
                | "turn_completed"
                | "response_error"
        )
    {
        let delta_raw = envelope
            .payload
            .get("content_delta")
            .and_then(|value| value.as_str());
        let content_raw = envelope
            .payload
            .get("content")
            .and_then(|value| value.as_str());
        let delta_len = envelope
            .payload
            .get("content_delta")
            .and_then(|value| value.as_str())
            .map(|value| value.len());
        let content_len = envelope
            .payload
            .get("content")
            .and_then(|value| value.as_str())
            .map(|value| value.len());
        let (delta_preview, delta_preview_truncated) = delta_raw
            .map(|value| log_preview(value, 24))
            .map(|(preview, truncated)| (Some(preview), Some(truncated)))
            .unwrap_or((None, None));
        let (content_preview, content_preview_truncated) = content_raw
            .map(|value| log_preview(value, 80))
            .map(|(preview, truncated)| (Some(preview), Some(truncated)))
            .unwrap_or((None, None));
        let assistant_len = public_turn_id
            .as_ref()
            .and_then(|turn_id| session.turns.get(turn_id))
            .and_then(|turn| turn.assistant_message.as_ref())
            .map(|value| value.len());
        let event_gap_ms = session
            .last_event_emitted_at
            .map(|ts| ts.elapsed().as_millis() as u64);
        info!(
            session_id = %session_id,
            turn_id = ?public_turn_id,
            event_type = event_type,
            event_seq = envelope.event_seq,
            turn_seq = envelope.turn_seq,
            stream_source = %envelope.stream_source,
            delta_len = ?delta_len,
            delta_preview = ?delta_preview,
            delta_preview_truncated = ?delta_preview_truncated,
            content_len = ?content_len,
            content_preview = ?content_preview,
            content_preview_truncated = ?content_preview_truncated,
            assistant_message_len = ?assistant_len,
            event_gap_ms = ?event_gap_ms,
            "stream_debug.gateway_event_emitted"
        );
    }
    if (event_type == "tool_call_started"
        || event_type == "tool_call_ready"
        || event_type == "tool_call_completed")
        && envelope.payload.get("tool_call_id").is_some()
    {
        info!(
            event_type = event_type,
            turn_id = ?public_turn_id,
            tool_call_id = ?envelope.payload.get("tool_call_id").and_then(|v| v.as_str()),
            "tool event mapped"
        );
    }
    session.last_event_emitted_at = Some(Instant::now());
    session.publish_event(envelope);

    public_turn_id
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
            | Event::ToolCallsProposed { turn_id, .. }
            | Event::ToolBatchCompleted { turn_id, .. }
            | Event::ResponseResumed { turn_id, .. }
            | Event::ResponseCompleted { turn_id, .. }
            | Event::ResponseError { turn_id, .. }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turn_status_remains_failed_after_turn_completed() {
        let mut session = SessionRuntime::new();
        let mut turn_id_tx = None;
        let _ = map_core_event(
            &mut session,
            "sess_1",
            "req_1",
            Event::ResponseError {
                turn_id: 1,
                code: "ERR".to_string(),
                message: "failed".to_string(),
                retryable: false,
            },
            &mut turn_id_tx,
        );
        let _ = map_core_event(
            &mut session,
            "sess_1",
            "req_1",
            Event::TurnCompleted { turn_id: 1 },
            &mut turn_id_tx,
        );
        let turn = session.turns.get("turn_1").expect("turn exists");
        assert_eq!(turn.status, TurnStatus::Failed);
    }

    #[test]
    fn turn_message_only_updates_from_response_completed() {
        let mut session = SessionRuntime::new();
        let mut turn_id_tx = None;
        let _ = map_core_event(
            &mut session,
            "sess_1",
            "req_1",
            Event::TurnStarted { turn_id: 1 },
            &mut turn_id_tx,
        );
        let _ = map_core_event(
            &mut session,
            "sess_1",
            "req_1",
            Event::AssistantMessage {
                turn_id: 1,
                content: "legacy".to_string(),
            },
            &mut turn_id_tx,
        );
        let turn = session.turns.get("turn_1").expect("turn exists");
        assert!(turn.assistant_message.is_none());

        let _ = map_core_event(
            &mut session,
            "sess_1",
            "req_1",
            Event::ResponseCompleted {
                turn_id: 1,
                content: "final".to_string(),
                stream_source: openjax_protocol::StreamSource::Synthetic,
            },
            &mut turn_id_tx,
        );
        let turn = session.turns.get("turn_1").expect("turn exists");
        assert_eq!(turn.assistant_message.as_deref(), Some("final"));
    }
}
