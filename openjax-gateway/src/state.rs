use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use async_trait::async_trait;
use openjax_core::{Agent, ApprovalHandler, ApprovalRequest, Config, approval_timeout_ms_from_env};
use openjax_protocol::Event;
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::{Mutex, RwLock, broadcast, mpsc, oneshot};
use tokio::time::{Duration, timeout};
use tracing::{info, warn};
use uuid::Uuid;

use crate::auth::load_api_keys_from_env;
use crate::error::{ApiError, now_rfc3339};

const EVENT_REPLAY_LIMIT: usize = 1024;

#[derive(Clone)]
pub struct AppState {
    sessions: Arc<RwLock<HashMap<String, Arc<Mutex<SessionRuntime>>>>>,
    api_keys: Arc<HashSet<String>>,
}

impl AppState {
    pub fn new() -> Self {
        Self::new_with_api_keys(load_api_keys_from_env())
    }

    pub fn new_with_api_keys(api_keys: HashSet<String>) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            api_keys: Arc::new(api_keys),
        }
    }

    pub fn is_api_key_allowed(&self, key: &str) -> bool {
        self.api_keys.contains(key)
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
    pub event_log: VecDeque<StreamEventEnvelope>,
    pub event_tx: broadcast::Sender<StreamEventEnvelope>,
    pub resolved_approvals: HashSet<String>,
}

impl SessionRuntime {
    pub fn new() -> Self {
        let mut agent = Agent::with_config(Config::load());
        let approval_handler = Arc::new(GatewayApprovalHandler::default());
        agent.set_approval_handler(approval_handler.clone());
        let (event_tx, _) = broadcast::channel(1024);
        Self {
            agent: Arc::new(Mutex::new(agent)),
            approval_handler,
            status: SessionStatus::Active,
            turns: HashMap::new(),
            core_turn_to_public: HashMap::new(),
            next_event_seq: 1,
            event_log: VecDeque::new(),
            event_tx,
            resolved_approvals: HashSet::new(),
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
    }

    pub fn create_gateway_event(
        &mut self,
        request_id: &str,
        session_id: &str,
        turn_id: Option<String>,
        event_type: &str,
        payload: Value,
    ) -> StreamEventEnvelope {
        let event = StreamEventEnvelope {
            request_id: request_id.to_string(),
            session_id: session_id.to_string(),
            turn_id,
            event_seq: self.next_event_seq,
            timestamp: now_rfc3339(),
            event_type: event_type.to_string(),
            payload,
        };
        self.next_event_seq += 1;
        event
    }

    pub fn publish_event(&mut self, event: StreamEventEnvelope) {
        if self.event_log.len() >= EVENT_REPLAY_LIMIT {
            self.event_log.pop_front();
        }
        self.event_log.push_back(event.clone());
        let _ = self.event_tx.send(event);
    }

    pub fn replay_from(
        &self,
        after_event_seq: Option<u64>,
    ) -> Result<Vec<StreamEventEnvelope>, ApiError> {
        let min_allowed = self
            .event_log
            .front()
            .map(|event| event.event_seq.saturating_sub(1))
            .unwrap_or(0);

        if let Some(seq) = after_event_seq
            && seq < min_allowed
        {
            return Err(ApiError::invalid_argument(
                "replay point is outside retention window",
                json!({ "after_event_seq": seq, "min_allowed": min_allowed }),
            ));
        }

        let events = self
            .event_log
            .iter()
            .filter(|event| after_event_seq.is_none_or(|seq| event.event_seq > seq))
            .cloned()
            .collect();
        Ok(events)
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
    pub timestamp: String,
    #[serde(rename = "type")]
    pub event_type: String,
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
    let (core_turn_id, event_type, payload) = match event {
        Event::TurnStarted { turn_id } => (Some(turn_id), "turn_started", json!({})),
        Event::AssistantDelta {
            turn_id,
            content_delta,
        } => (
            Some(turn_id),
            "assistant_delta",
            json!({ "content_delta": content_delta }),
        ),
        Event::AssistantMessage { turn_id, content } => (
            Some(turn_id),
            "assistant_message",
            json!({ "content": content }),
        ),
        Event::ToolCallStarted {
            turn_id,
            tool_call_id,
            tool_name,
            target,
        } => (
            Some(turn_id),
            "tool_call_started",
            json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "target": target }),
        ),
        Event::ToolCallCompleted {
            turn_id,
            tool_call_id,
            tool_name,
            ok,
            output,
        } => (
            Some(turn_id),
            "tool_call_completed",
            json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "ok": ok, "output": output }),
        ),
        Event::ApprovalRequested {
            turn_id,
            request_id: approval_id,
            target,
            reason,
            tool_name,
            command_preview,
            risk_tags,
            sandbox_backend,
            degrade_reason,
        } => (
            Some(turn_id),
            "approval_requested",
            json!({
                "approval_id": approval_id,
                "target": target,
                "reason": reason,
                "tool_name": tool_name,
                "command_preview": command_preview,
                "risk_tags": risk_tags,
                "sandbox_backend": sandbox_backend,
                "degrade_reason": degrade_reason
            }),
        ),
        Event::ApprovalResolved {
            turn_id,
            request_id: approval_id,
            approved,
        } => (
            Some(turn_id),
            "approval_resolved",
            json!({
                "approval_id": approval_id,
                "approved": approved
            }),
        ),
        Event::TurnCompleted { turn_id } => (Some(turn_id), "turn_completed", json!({})),
        Event::ShutdownComplete => (None, "session_shutdown", json!({})),
        Event::AgentSpawned { .. } | Event::AgentStatusChanged { .. } => return None,
    };

    let public_turn_id = core_turn_id.map(|tid| session.get_or_create_public_turn_id(tid));
    if let Some(turn_id) = &public_turn_id {
        let turn = session
            .turns
            .entry(turn_id.clone())
            .or_insert_with(TurnRuntime::queued);
        if event_type == "turn_started" {
            turn.status = TurnStatus::Running;
        } else if event_type == "turn_completed" {
            turn.status = TurnStatus::Completed;
        } else if event_type == "assistant_message"
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
    );
    if (event_type == "tool_call_started" || event_type == "tool_call_completed")
        && envelope.payload.get("tool_call_id").is_some()
    {
        info!(
            event_type = event_type,
            turn_id = ?public_turn_id,
            tool_call_id = ?envelope.payload.get("tool_call_id").and_then(|v| v.as_str()),
            "tool event mapped"
        );
    }
    session.publish_event(envelope);

    public_turn_id
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
